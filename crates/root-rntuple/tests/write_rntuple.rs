//! M5: write a scalar RNTuple and read it back through our own reader.

use std::path::PathBuf;

use root_io_core::RFile;
use root_rntuple::{ColumnValues, RNTuple, ScalarColumn, ScalarField};

#[test]
fn writes_a_scalar_rntuple_that_round_trips() {
    let fields = vec![
        ScalarField {
            name: "i32".into(),
            data: ScalarColumn::I32(vec![0, 10, 20, 30, 40]),
        },
        ScalarField {
            name: "f32".into(),
            data: ScalarColumn::F32(vec![0.5, 1.5, 2.5, 3.5, 4.5]),
        },
        ScalarField {
            name: "f64".into(),
            data: ScalarColumn::F64(vec![0.0, 1.25, 2.5, 3.75, 5.0]),
        },
    ];

    let out = PathBuf::from("/tmp/rootrs_written_ntuple.root");
    root_rntuple::write_rntuple_file(&out, "ntpl", &fields).expect("write rntuple");

    let f = RFile::open(&out).expect("reopen");
    assert_eq!(f.key("ntpl").unwrap().class_name, "ROOT::RNTuple");

    let ntpl = RNTuple::open(&f, "ntpl").expect("open RNTuple");
    assert_eq!(ntpl.num_entries(), 5);

    let names: Vec<&str> = ntpl
        .header()
        .fields
        .iter()
        .map(|fd| fd.name.as_str())
        .collect();
    assert_eq!(names, ["i32", "f32", "f64"]);

    assert_eq!(
        ntpl.read_column(&f, 0).unwrap(),
        ColumnValues::I32(vec![0, 10, 20, 30, 40])
    );
    assert_eq!(
        ntpl.read_column(&f, 1).unwrap(),
        ColumnValues::F32(vec![0.5, 1.5, 2.5, 3.5, 4.5])
    );
    assert_eq!(
        ntpl.read_column(&f, 2).unwrap(),
        ColumnValues::F64(vec![0.0, 1.25, 2.5, 3.75, 5.0])
    );
}
