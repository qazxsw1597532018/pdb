extern crate pdb;
extern crate uuid;
extern crate curl;

use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Once, ONCE_INIT};
use pdb::FallibleIterator;

// This test is intended to cover OMAP address translation:
//   https://github.com/willglynn/pdb/issues/17

static DOWNLOADED: Once = ONCE_INIT;
fn open_file() -> std::fs::File {
    let path = "fixtures/symbol_server/3844dbb920174967be7aa4a2c20430fa2-ntkrnlmp.pdb";
    let url = "https://msdl.microsoft.com/download/symbols/ntkrnlmp.pdb/3844dbb920174967be7aa4a2c20430fa2/ntkrnlmp.pdb";

    std::fs::File::open(path)
        .unwrap_or_else(|_| {
            // download once, even if we're called concurrently
            DOWNLOADED.call_once(|| {
                let mut temporary_path = PathBuf::from(path);
                temporary_path.set_extension("tmp");

                {
                    // download to a temporary file
                    let mut temporary_file = std::fs::File::create(&temporary_path).unwrap();

                    // ask curl to do the download

                    let mut req = curl::easy::Easy::new();
                    req.url(url).unwrap();
                    req.write_function(move |data| {
                        Ok(temporary_file.write(data).expect("write data"))
                    }).unwrap();
                    req.perform().expect("download");

                    // close the temporary file, because Windows
                }

                // rename and reopen
                std::fs::rename(temporary_path, path).unwrap();
            });

            std::fs::File::open(path).expect("open PDB after downloading")
        })
}

#[test]
fn verify_pdb_identity() {
    // make sure this is the right PDB
    let mut pdb = pdb::PDB::open(open_file()).expect("opening pdb");

    let pdb_info = pdb.pdb_information().expect("pdb information");
    assert_eq!(pdb_info.guid, uuid::Uuid::from_str("3844DBB9-2017-4967-BE7A-A4A2C20430FA").unwrap());
    assert_eq!(pdb_info.age, 5);
    assert_eq!(pdb_info.signature, 1290245416);
}

#[test]
fn test_omap() {
    let mut pdb = pdb::PDB::open(open_file()).expect("opening pdb");

    let global_symbols = pdb.global_symbols().expect("global_symbols");

    // find the target symbol
    let target_symbol = {
        let target_name = pdb::RawString::from("NtWaitForSingleObject");
        let mut iter = global_symbols.iter();
        iter.filter(|sym| sym.name().expect("symbol name") == target_name).next()
            .expect("iterate symbols")
            .expect("find target symbol")
    };

    // extract the PublicSymbol data
    let pubsym = match target_symbol.parse().expect("parse symbol") {
        pdb::SymbolData::PublicSymbol(pubsym) => pubsym,
        _ => panic!("expected public symbol")
    };

    // ensure the symbol has the correct location
    assert_eq!(pubsym.segment, 0x000c);
    assert_eq!(pubsym.offset, 0x0004aeb0);

    // great, good to go
    // find the debug information
    pdb.debug_information().expect("debug_information");

    // TODO:
    //   build an address translator
    //   translate the segment+offset
    //   assert_eq!(rva, 0x003768c0)
}
