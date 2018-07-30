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

use std::fmt;

// Messages related to PBFT consensus
#[derive(Debug, PartialEq, PartialOrd)]
pub enum PbftMessageType {
    // Basic message types for the multicast protocol
    PrePrepare,
    Prepare,
    Commit,

    // Auxiliary PBFT messages
    BlockNew,
    Checkpoint,
    ViewChange,

    Unset,
}

impl fmt::Display for PbftMessageType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let txt = match self {
            PbftMessageType::PrePrepare => "PP",
            PbftMessageType::Prepare => "Pr",
            PbftMessageType::Commit => "Co",
            PbftMessageType::BlockNew => "BN",
            PbftMessageType::Checkpoint => "CP",
            PbftMessageType::ViewChange => "VC",
            PbftMessageType::Unset => "Un",
        };
        write!(f, "{}", txt)
    }
}

impl PbftMessageType {
    pub fn is_multicast(&self) -> bool {
        match self {
            PbftMessageType::PrePrepare | PbftMessageType::Prepare | PbftMessageType::Commit => {
                true
            }
            _ => false,
        }
    }
}

impl<'a> From<&'a str> for PbftMessageType {
    fn from(s: &'a str) -> Self {
        match s {
            "PrePrepare" => PbftMessageType::PrePrepare,
            "Prepare" => PbftMessageType::Prepare,
            "Commit" => PbftMessageType::Commit,
            "BlockNew" => PbftMessageType::BlockNew,
            "ViewChange" => PbftMessageType::ViewChange,
            "Checkpoint" => PbftMessageType::Checkpoint,
            _ => {
                warn!("Unhandled PBFT message type: {}", s);
                PbftMessageType::Unset
            }
        }
    }
}

impl<'a> From<&'a PbftMessageType> for String {
    fn from(mc_type: &'a PbftMessageType) -> String {
        format!("{:?}", mc_type)
    }
}
