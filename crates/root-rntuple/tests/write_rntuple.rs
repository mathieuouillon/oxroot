//! M5/#2: write an RNTuple with scalar, string, and vector fields, then read it
//! back through our own reader.

use std::path::PathBuf;

use root_io_core::RFile;
use root_rntuple::{Column, Field, FieldValues, RNTuple};

#[test]
fn writes_a_rich_rntuple_that_round_trips() {
    let fields = vec![
        Field {
            name: "i32".into(),
            data: Column::I32(vec![0, 10, 20, 30, 40]),
        },
        Field {
            name: "i64".into(),
            data: Column::I64(vec![-1, -2, -3, -4, -5]),
        },
        Field {
            name: "f32".into(),
            data: Column::F32(vec![0.5, 1.5, 2.5, 3.5, 4.5]),
        },
        Field {
            name: "f64".into(),
            data: Column::F64(vec![0.0, 1.25, 2.5, 3.75, 5.0]),
        },
        Field {
            name: "b".into(),
            data: Column::Bool(vec![true, false, true, false, true]),
        },
        Field {
            name: "s".into(),
            data: Column::Str(
                ["row0", "row1", "row2", "row3", "row4"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
        },
        Field {
            name: "vf".into(),
            data: Column::VecF32(vec![
                vec![],
                vec![1.0],
                vec![2.0, 2.0],
                vec![3.0, 3.0, 3.0],
                vec![4.0, 4.0, 4.0, 4.0],
            ]),
        },
    ];

    let out = PathBuf::from("/tmp/rootrs_written_rich_ntuple.root");
    root_rntuple::write_rntuple_file(&out, "ntpl", &fields).expect("write rntuple");

    let f = RFile::open(&out).expect("reopen");
    let ntpl = RNTuple::open(&f, "ntpl").expect("open RNTuple");
    assert_eq!(ntpl.num_entries(), 5);

    let field = |n| ntpl.read_field(&f, n).expect("read field");
    assert_eq!(field("i32"), FieldValues::I32(vec![0, 10, 20, 30, 40]));
    assert_eq!(field("i64"), FieldValues::I64(vec![-1, -2, -3, -4, -5]));
    assert_eq!(
        field("f32"),
        FieldValues::F32(vec![0.5, 1.5, 2.5, 3.5, 4.5])
    );
    assert_eq!(
        field("f64"),
        FieldValues::F64(vec![0.0, 1.25, 2.5, 3.75, 5.0])
    );
    assert_eq!(
        field("b"),
        FieldValues::Bool(vec![true, false, true, false, true])
    );
    assert_eq!(
        field("s"),
        FieldValues::Str(
            ["row0", "row1", "row2", "row3", "row4"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        )
    );
    assert_eq!(
        field("vf"),
        FieldValues::VecF32(vec![
            vec![],
            vec![1.0],
            vec![2.0, 2.0],
            vec![3.0, 3.0, 3.0],
            vec![4.0, 4.0, 4.0, 4.0],
        ])
    );
}
