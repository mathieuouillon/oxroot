// Dev-only RNTuple fixture generator (requires ROOT 6.34+). uproot can read
// RNTuple but not write it, so reference files come from ROOT itself.
//
//   c++ $(root-config --cflags) scripts/gen_rntuple_fixtures.cpp \
//       $(root-config --libs) -lROOTNTuple -o /tmp/gen_rntuple
//   /tmp/gen_rntuple        # run from the repo root
//
// Writes the same dataset twice: uncompressed (non-split column types) and
// Zstd-compressed (split column types), to exercise both decode paths.

#include <cstdint>
#include <string>
#include <vector>

#include <ROOT/RNTupleModel.hxx>
#include <ROOT/RNTupleWriteOptions.hxx>
#include <ROOT/RNTupleWriter.hxx>

static void write_rntuple(const char *path, int compression) {
    auto model = ROOT::RNTupleModel::Create();
    auto fI32 = model->MakeField<std::int32_t>("i32");
    auto fF32 = model->MakeField<float>("f32");
    auto fF64 = model->MakeField<double>("f64");
    auto fBool = model->MakeField<bool>("b");
    auto fStr = model->MakeField<std::string>("s");
    auto fVec = model->MakeField<std::vector<float>>("vf");

    ROOT::RNTupleWriteOptions options;
    options.SetCompression(compression); // 0 = none, 505 = Zstd level 5

    auto writer = ROOT::RNTupleWriter::Recreate(std::move(model), "ntpl", path, options);

    for (int i = 0; i < 5; ++i) {
        *fI32 = i * 10;
        *fF32 = static_cast<float>(i) + 0.5f;
        *fF64 = static_cast<double>(i) * 1.25;
        *fBool = (i % 2 == 0);
        *fStr = "row" + std::to_string(i);
        fVec->assign(static_cast<std::size_t>(i), static_cast<float>(i)); // length i, all = i
        writer->Fill();
    }
}

int main() {
    write_rntuple("fixtures/rntuple_scalars_uncompressed.root", 0);
    write_rntuple("fixtures/rntuple_scalars_zstd.root", 505);
    return 0; // each writer's destructor commits its dataset
}
