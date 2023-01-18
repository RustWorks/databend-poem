// Copyright 2022 Datafuse Labs.
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

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use common_arrow::arrow::datatypes::Schema as ArrowSchema;
use common_arrow::arrow::io::parquet::write::to_parquet_schema;
use common_arrow::parquet::metadata::ColumnDescriptor;
use common_arrow::schema_projection as ap;
use common_catalog::plan::Projection;
use common_exception::Result;
use common_expression::DataSchema;
use common_expression::DataSchemaRef;
use common_storage::ColumnNodes;
use opendal::Object;
use opendal::Operator;

use crate::parquet_part::ParquetRowGroupPart;
use crate::table_function::arrow_to_table_schema;

pub type IndexedChunk = (usize, Vec<u8>);

/// The reader to parquet files with a projected schema.
///
/// **ALERT**: dictionary type is not supported yet.
/// If there are dictionary pages in the parquet file, the reading process may fail.
#[derive(Clone)]
pub struct ParquetReader {
    operator: Operator,
    /// The indices of columns need to read by this reader.
    ///
    /// Use [`HashSet`] to avoid duplicate indices.
    /// Duplicate indices will exist when there are nested types or
    /// select a same field multiple times.
    ///
    /// For example:
    ///
    /// ```sql
    /// select a, a.b, a.c from t;
    /// select a, b, a from t;
    /// ```
    columns_to_read: HashSet<usize>,
    /// The schema of the [`common_expression::DataBlock`] this reader produces.
    ///
    /// ```
    /// output_schema = DataSchema::from(projected_arrow_schema)
    /// ```
    pub(crate) output_schema: DataSchemaRef,
    /// The actual schema used to read parquet.
    ///
    /// The reason of using [`ArrowSchema`] to read parquet is that
    /// There are some types that Databend not support such as Timestamp of nanoseconds.
    /// Such types will be convert to supported types after deserialization.
    pub(crate) projected_arrow_schema: ArrowSchema,
    /// [`ColumnNodes`] corresponding to the `projected_arrow_schema`.
    pub(crate) projected_column_nodes: ColumnNodes,
    /// [`ColumnDescriptor`]s corresponding to the `projected_arrow_schema`.
    pub(crate) projected_column_descriptors: HashMap<usize, ColumnDescriptor>,
}

impl ParquetReader {
    pub fn create(
        operator: Operator,
        schema: ArrowSchema,
        projection: Projection,
    ) -> Result<Arc<ParquetReader>> {
        let (
            projected_arrow_schema,
            projected_column_nodes,
            projected_column_descriptors,
            columns_to_read,
        ) = Self::do_projection(&schema, &projection)?;

        let t_schema = arrow_to_table_schema(projected_arrow_schema.clone());
        let output_schema = DataSchema::from(&t_schema);

        Ok(Arc::new(ParquetReader {
            operator,
            columns_to_read,
            output_schema: Arc::new(output_schema),
            projected_arrow_schema,
            projected_column_nodes,
            projected_column_descriptors,
        }))
    }

    pub fn output_schema(&self) -> &DataSchema {
        &self.output_schema
    }

    pub fn columns_to_read(&self) -> &HashSet<usize> {
        &self.columns_to_read
    }

    /// Project the schema and get the needed column leaves.
    #[allow(clippy::type_complexity)]
    pub fn do_projection(
        schema: &ArrowSchema,
        projection: &Projection,
    ) -> Result<(
        ArrowSchema,
        ColumnNodes,
        HashMap<usize, ColumnDescriptor>,
        HashSet<usize>,
    )> {
        // Full schema and column leaves.
        let column_nodes = ColumnNodes::new_from_schema(schema);
        let schema_descriptors = to_parquet_schema(schema)?;
        // Project schema
        let projected_arrow_schema = match projection {
            Projection::Columns(indices) => ap::project(schema, indices),
            Projection::InnerColumns(path_indices) => ap::inner_project(schema, path_indices),
        };
        // Project column leaves
        let projected_column_nodes = ColumnNodes {
            column_nodes: projection
                .project_column_nodes(&column_nodes)?
                .iter()
                .map(|&leaf| leaf.clone())
                .collect(),
        };
        let column_nodes = &projected_column_nodes.column_nodes;
        // Project column descriptors and collect columns to read
        let mut projected_column_descriptors = HashMap::with_capacity(column_nodes.len());
        let mut columns_to_read =
            HashSet::with_capacity(column_nodes.iter().map(|leaf| leaf.leaf_ids.len()).sum());
        for column_node in column_nodes {
            for index in &column_node.leaf_ids {
                columns_to_read.insert(*index);
                projected_column_descriptors
                    .insert(*index, schema_descriptors.columns()[*index].clone());
            }
        }
        Ok((
            projected_arrow_schema,
            projected_column_nodes,
            projected_column_descriptors,
            columns_to_read,
        ))
    }

    /// Read columns data of one row group.
    pub fn sync_read_columns(&self, part: &ParquetRowGroupPart) -> Result<Vec<IndexedChunk>> {
        let mut chunks = Vec::with_capacity(self.columns_to_read.len());

        for index in &self.columns_to_read {
            let meta = &part.column_metas[index];
            let op = self.operator.clone();
            let chunk =
                Self::sync_read_one_column(op.object(&part.location), meta.offset, meta.length)?;
            chunks.push((*index, chunk));
        }

        Ok(chunks)
    }

    #[inline]
    pub fn sync_read_one_column(o: Object, offset: u64, length: u64) -> Result<Vec<u8>> {
        Ok(o.blocking_range_read(offset..offset + length)?)
    }
}
