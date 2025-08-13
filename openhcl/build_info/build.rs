// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#![expect(missing_docs)]

fn main() {
    // Set build timestamp for reproducible builds
    // If SOURCE_DATE_EPOCH is set, use it for reproducible builds
    // Otherwise, use current time for development builds
    if let Ok(source_date_epoch) = std::env::var("SOURCE_DATE_EPOCH") {
        println!("cargo:rustc-env=BUILD_TIMESTAMP={}", source_date_epoch);
    } else {
        // Fallback to current time for development builds
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        println!("cargo:rustc-env=BUILD_TIMESTAMP={}", now);
    }

    vergen::EmitBuilder::builder().all_git().emit().unwrap();
}
