// This file is part of Gear.

// Copyright (C) 2023 Gear Technologies Inc.
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

use codec::{Decode, Encode};
use core::{future::Future, pin::Pin};
use futures::FutureExt;
use gstd::{msg, prelude::*};

#[derive(Debug, codec::Decode)]
pub struct ControlSignal {
    pub deployed_actor: gstd::ActorId,
}

#[derive(Debug, codec::Encode)]
pub enum ProgressSignal {
    TestStart(String),
    TestSuccess(String),
}

#[no_mangle]
pub unsafe extern "C" fn handle() {
    let payload = msg::load_bytes().expect("Failed to load payload");

    if payload == b"PING" {
        msg::reply_bytes("PONG", 0).expect("Failed to send reply");
    }
}

struct TestContext {
    deployed_actor: gstd::ActorId,
    control_bus: gstd::ActorId,
}

impl TestContext {
    fn current() -> Self {
        let req = msg::load::<ControlSignal>().expect("Failed to decode control signal");

        TestContext {
            deployed_actor: req.deployed_actor,
            control_bus: msg::source(),
        }
    }

    fn send_progress(&self, msg: ProgressSignal) {
        msg::send(self.deployed_actor, msg, 0);
    }

    fn test_start(&self, name: &str) {
        self.send_progress(ProgressSignal::TestStart(name.to_string()))
    }

    fn test_success(&self, name: &str) {
        self.send_progress(ProgressSignal::TestSuccess(name.to_string()))
    }
}

type PinnedFuture = Pin<Box<dyn Future<Output = ()> + 'static>>;

// thread-local-like variable for run_tests workflow (synchronously populating one big future)
static mut CONTEXT_FUTURES: Vec<PinnedFuture> = Vec::new();

pub unsafe extern "C" fn test_smoky() {
    let test_future = async {
        // test preamble
        let context = TestContext::current();
        context.test_start("test_smoky");

        // test body
        {
            assert!(1 == 1);
        }

        // test epilogue
        context.test_success("test_smoky");
    }
    .boxed();

    unsafe {
        CONTEXT_FUTURES.push(test_future);
    }
}

#[no_mangle]
pub unsafe extern "C" fn run_tests(ptr: *const u8) {
    // at the moment, just runs all tests

    // invoke all declared tests..

    // drain message to local var and create FuturesUnordered

    // run message loop based on what we produced
}
