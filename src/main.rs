/*
 * Copyright 2018 Bitwise IO, Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 * -----------------------------------------------------------------------------
 */

#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;
extern crate hex;
extern crate protobuf;
extern crate sawtooth_sdk;
extern crate serde_json;
extern crate simple_logger;

use std::process;

use sawtooth_sdk::consensus::zmq_driver::ZmqDriver;
use sawtooth_sdk::consensus::engine::Engine;

mod node;
mod protos;
mod crashing_node;
mod normal_node;

fn main() {
    let matches = clap_app!(sawtooth_pbft =>
        (version: crate_version!())
        (about: "PBFT consensus for Sawtooth")
        (@arg connect: -C --connect +takes_value
         "connection endpoint for validator")
        (@arg verbose: -v --verbose +multiple
         "increase output verbosity")
        (@arg ID: +required "the PBFT node's id")
        (@arg dead: -d +takes_value "simulate a dead node"))
        .get_matches();

    let log_level = match matches.occurrences_of("verbose") {
        0 => log::Level::Warn,
        1 => log::Level::Info,
        2 => log::Level::Debug,
        3 | _ => log::Level::Trace,
    };

    let endpoint = String::from(
        matches
            .value_of("connect")
            .unwrap_or("tcp://localhost:5050"),
    );

    let id = value_t!(matches.value_of("ID"), u64).unwrap_or_else(|e| e.exit());
    let dead = value_t!(matches.value_of("dead"), isize).unwrap_or(-1);

    simple_logger::init_with_level(log_level).unwrap();

    warn!("Sawtooth PBFT Engine ({})", env!("CARGO_PKG_VERSION"));

    let (driver, _stop) = ZmqDriver::new();

    warn!("PBFT Node {} connecting to '{}'", &id, &endpoint);
    if dead >= 0 {
        warn!("    This node will be dead after {} seconds", dead);
        let pbft_engine = crashing_node::engine::PbftEngine::new(id, dead);
        driver.start(&endpoint, pbft_engine).unwrap_or_else(|err| {
            error!("{}", err);
            process::exit(1);
        });
    } else {
        let pbft_engine = normal_node::engine::PbftEngine::new(id);
        driver.start(&endpoint, pbft_engine).unwrap_or_else(|err| {
            error!("{}", err);
            process::exit(1);
        });
    }

}
