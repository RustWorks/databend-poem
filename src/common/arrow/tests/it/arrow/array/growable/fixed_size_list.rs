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

use common_arrow::arrow::array::growable::Growable;
use common_arrow::arrow::array::growable::GrowableFixedSizeList;
use common_arrow::arrow::array::FixedSizeListArray;
use common_arrow::arrow::array::MutableFixedSizeListArray;
use common_arrow::arrow::array::MutablePrimitiveArray;
use common_arrow::arrow::array::TryExtend;

fn create_list_array(data: Vec<Option<Vec<Option<i32>>>>) -> FixedSizeListArray {
    let mut array = MutableFixedSizeListArray::new(MutablePrimitiveArray::<i32>::new(), 3);
    array.try_extend(data).unwrap();
    array.into()
}

#[test]
fn basic() {
    let data = vec![
        Some(vec![Some(1i32), Some(2), Some(3)]),
        Some(vec![Some(4), Some(5), Some(6)]),
        Some(vec![Some(7i32), Some(8), Some(9)]),
    ];

    let array = create_list_array(data);

    let mut a = GrowableFixedSizeList::new(vec![&array], false, 0);
    a.extend(0, 0, 1);
    assert_eq!(a.len(), 1);

    let result: FixedSizeListArray = a.into();

    let expected = vec![Some(vec![Some(1i32), Some(2), Some(3)])];
    let expected = create_list_array(expected);

    assert_eq!(result, expected)
}

#[test]
fn null_offset() {
    let data = vec![
        Some(vec![Some(1i32), Some(2), Some(3)]),
        None,
        Some(vec![Some(6i32), Some(7), Some(8)]),
    ];
    let array = create_list_array(data);
    let array = array.sliced(1, 2);

    let mut a = GrowableFixedSizeList::new(vec![&array], false, 0);
    a.extend(0, 1, 1);
    assert_eq!(a.len(), 1);

    let result: FixedSizeListArray = a.into();

    let expected = vec![Some(vec![Some(6i32), Some(7), Some(8)])];
    let expected = create_list_array(expected);

    assert_eq!(result, expected)
}

#[test]
fn test_from_two_lists() {
    let data_1 = vec![
        Some(vec![Some(1i32), Some(2), Some(3)]),
        None,
        Some(vec![Some(6i32), None, Some(8)]),
    ];
    let array_1 = create_list_array(data_1);

    let data_2 = vec![
        Some(vec![Some(8i32), Some(7), Some(6)]),
        Some(vec![Some(5i32), None, Some(4)]),
        Some(vec![Some(2i32), Some(1), Some(0)]),
    ];
    let array_2 = create_list_array(data_2);

    let mut a = GrowableFixedSizeList::new(vec![&array_1, &array_2], false, 6);
    a.extend(0, 0, 2);
    a.extend(1, 1, 1);
    assert_eq!(a.len(), 3);

    let result: FixedSizeListArray = a.into();

    let expected = vec![
        Some(vec![Some(1i32), Some(2), Some(3)]),
        None,
        Some(vec![Some(5i32), None, Some(4)]),
    ];
    let expected = create_list_array(expected);

    assert_eq!(result, expected);
}
