// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! A command line tool for inspecting IGVM files.
//!
//! Provides `dump` and `diff` subcommands for examining and comparing IGVM
//! files. For generating IGVM files, see `igvmfilegen`.

#![forbid(unsafe_code)]

mod diff;

use anyhow::Context;
use clap::Parser;
use igvm::IgvmFile;
use igvm_defs::IGVM_FIXED_HEADER;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use zerocopy::FromBytes;
use zerocopy::IntoBytes;

#[derive(Parser)]
#[clap(name = "igvminspect", about = "Tool to inspect IGVM files")]
enum Options {
    /// Dumps the contents of an IGVM file in a human-readable format
    Dump {
        /// Dump file path
        #[clap(short, long = "filepath")]
        file_path: PathBuf,
    },
    /// Diff two IGVM files by extracting parts and running diffoscope
    Diff {
        /// First IGVM file
        #[clap(short, long)]
        left: PathBuf,
        /// Second IGVM file
        #[clap(short, long)]
        right: PathBuf,
        /// Map file (.bin.map) for the first IGVM file
        #[clap(long)]
        left_map: PathBuf,
        /// Map file (.bin.map) for the second IGVM file
        #[clap(long)]
        right_map: PathBuf,
        /// Keep extracted temp directories after diffoscope exits
        #[clap(long)]
        keep_extracted: bool,
        /// Additional arguments to pass to diffoscope
        #[clap(last = true)]
        diffoscope_args: Vec<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let opts = Options::parse();
    let filter = if std::env::var(EnvFilter::DEFAULT_ENV).is_ok() {
        EnvFilter::from_default_env()
    } else {
        EnvFilter::default().add_directive(LevelFilter::INFO.into())
    };
    tracing_subscriber::fmt()
        .log_internal_errors(true)
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .init();

    match opts {
        Options::Dump { file_path } => {
            let image = fs_err::read(file_path).context("reading input file")?;
            let fixed_header = IGVM_FIXED_HEADER::read_from_prefix(image.as_bytes())
                .expect("Invalid fixed header")
                .0; // TODO: zerocopy: use-rest-of-range (https://github.com/microsoft/openvmm/issues/759)

            let igvm_data = IgvmFile::new_from_binary(&image, None).expect("should be valid");
            println!("Total file size: {} bytes\n", fixed_header.total_file_size);
            println!("{:#X?}", fixed_header);
            println!("{}", igvm_data);
            Ok(())
        }
        Options::Diff {
            left,
            right,
            left_map,
            right_map,
            keep_extracted,
            diffoscope_args,
        } => diff::diff_igvm_files(
            &left,
            &right,
            &left_map,
            &right_map,
            keep_extracted,
            &diffoscope_args,
        ),
    }
}
