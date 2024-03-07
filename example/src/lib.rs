// This file is part of Gear.

// Copyright (C) 2021-2023 Gear Technologies Inc.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![no_std]

#[cfg(feature = "std")]
mod code {
    include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));
}

#[cfg(feature = "std")]
pub use code::WASM_BINARY_OPT as WASM_BINARY;

#[cfg(not(feature = "std"))]
mod wasm;

#[cfg(test)]
mod tests {
    extern crate std;

    use gear_test_runtime::ControlSignal;
    use gtest::{Program, System};

    #[test]
    fn program_can_be_initialized() {
        let system = System::new();
        system.init_logger();

        // test_program
        let test_program = Program::from_file(
            &system,
            "../target/wasm32-unknown-unknown/debug/example_test.opt.wasm",
        );
        let res = test_program.send_bytes(0, b"dummy");
        assert!(!res.main_failed());

        // actual program
        let code_hash =
            system.submit_code("../target/wasm32-unknown-unknown/debug/example.opt.wasm");

        // actual test run
        let res = test_program.send(
            0,
            ControlSignal::Test {
                code_hash: code_hash.into_bytes().into(),
                control_bus: test_program.id().into_bytes().into(),
            },
        );
        assert!(!res.main_failed());
    }
}
