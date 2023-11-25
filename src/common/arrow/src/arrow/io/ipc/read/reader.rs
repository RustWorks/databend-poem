// Copyright 2020-2022 Jorge C. Leitão
// Copyright 2021 Datafuse Labs
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::io::Read;
use std::io::Seek;

use ahash::AHashMap;

use super::common::*;
use super::read_batch;
use super::read_file_dictionaries;
use super::Dictionaries;
use super::FileMetadata;
use crate::arrow::array::Array;
use crate::arrow::chunk::Chunk;
use crate::arrow::datatypes::Schema;
use crate::arrow::error::Result;

/// An iterator of [`Chunk`]s from an Arrow IPC file.
pub struct FileReader<R: Read + Seek> {
    reader: R,
    metadata: FileMetadata,
    // the dictionaries are going to be read
    dictionaries: Option<Dictionaries>,
    current_block: usize,
    projection: Option<(Vec<usize>, AHashMap<usize, usize>, Schema)>,
    remaining: usize,
    data_scratch: Vec<u8>,
    message_scratch: Vec<u8>,
}

impl<R: Read + Seek> FileReader<R> {
    /// Creates a new [`FileReader`]. Use `projection` to only take certain columns.
    /// # Panic
    /// Panics iff the projection is not in increasing order (e.g. `[1, 0]` nor `[0, 1, 1]` are valid)
    pub fn new(
        reader: R,
        metadata: FileMetadata,
        projection: Option<Vec<usize>>,
        limit: Option<usize>,
    ) -> Self {
        let projection = projection.map(|projection| {
            let (p, h, fields) = prepare_projection(&metadata.schema.fields, projection);
            let schema = Schema {
                fields,
                metadata: metadata.schema.metadata.clone(),
            };
            (p, h, schema)
        });
        Self {
            reader,
            metadata,
            dictionaries: Default::default(),
            projection,
            remaining: limit.unwrap_or(usize::MAX),
            current_block: 0,
            data_scratch: Default::default(),
            message_scratch: Default::default(),
        }
    }

    /// Return the schema of the file
    pub fn schema(&self) -> &Schema {
        self.projection
            .as_ref()
            .map(|x| &x.2)
            .unwrap_or(&self.metadata.schema)
    }

    /// Returns the [`FileMetadata`]
    pub fn metadata(&self) -> &FileMetadata {
        &self.metadata
    }

    /// Consumes this FileReader, returning the underlying reader
    pub fn into_inner(self) -> R {
        self.reader
    }

    /// Get the inner memory scratches so they can be reused in a new writer.
    /// This can be utilized to save memory allocations for performance reasons.
    pub fn get_scratches(&mut self) -> (Vec<u8>, Vec<u8>) {
        (
            std::mem::take(&mut self.data_scratch),
            std::mem::take(&mut self.message_scratch),
        )
    }

    /// Set the inner memory scratches so they can be reused in a new writer.
    /// This can be utilized to save memory allocations for performance reasons.
    pub fn set_scratches(&mut self, scratches: (Vec<u8>, Vec<u8>)) {
        (self.data_scratch, self.message_scratch) = scratches;
    }

    fn read_dictionaries(&mut self) -> Result<()> {
        if self.dictionaries.is_none() {
            self.dictionaries = Some(read_file_dictionaries(
                &mut self.reader,
                &self.metadata,
                &mut self.data_scratch,
            )?);
        };
        Ok(())
    }
}

impl<R: Read + Seek> Iterator for FileReader<R> {
    type Item = Result<Chunk<Box<dyn Array>>>;

    fn next(&mut self) -> Option<Self::Item> {
        // get current block
        if self.current_block == self.metadata.blocks.len() {
            return None;
        }

        match self.read_dictionaries() {
            Ok(_) => {}
            Err(e) => return Some(Err(e)),
        };

        let block = self.current_block;
        self.current_block += 1;

        let chunk = read_batch(
            &mut self.reader,
            self.dictionaries.as_ref().unwrap(),
            &self.metadata,
            self.projection.as_ref().map(|x| x.0.as_ref()),
            Some(self.remaining),
            block,
            &mut self.message_scratch,
            &mut self.data_scratch,
        );
        self.remaining -= chunk.as_ref().map(|x| x.len()).unwrap_or_default();

        let chunk = if let Some((_, map, _)) = &self.projection {
            // re-order according to projection
            chunk.map(|chunk| apply_projection(chunk, map))
        } else {
            chunk
        };
        Some(chunk)
    }
}
