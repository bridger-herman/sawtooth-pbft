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

use std::collections::HashMap;
use std::fmt;

use hex;

use sawtooth_sdk::consensus::engine::{PeerId, BlockId};

use protos::pbft_message::PbftBlock;

use node::config::PbftConfig;
use node::message_type::PbftMessageType;
use node::timing::Timeout;
use node::error::PbftError;

// Possible roles for a node
// Primary is in charge of making consensus decisions
#[derive(Debug, PartialEq)]
enum PbftNodeRole {
    Primary,
    Secondary,
}

// Stages of the PBFT algorithm
#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub enum PbftPhase {
    NotStarted,
    PrePreparing,
    Preparing,
    Checking,
    Committing,
    Finished,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum PbftMode {
    Normal,
    ViewChanging,
    Checkpointing,
}

impl fmt::Display for PbftState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ast = if self.is_primary() { "*" } else { " " };
        let mode = match self.mode {
            PbftMode::Normal => "N",
            PbftMode::Checkpointing => "C",
            PbftMode::ViewChanging => "V",
        };

        let phase = match self.phase {
            PbftPhase::NotStarted => "NS",
            PbftPhase::PrePreparing => "PP",
            PbftPhase::Preparing => "Pr",
            PbftPhase::Checking => "Ch",
            PbftPhase::Committing => "Co",
            PbftPhase::Finished => "Fi",
        };

        let wb = match self.working_block {
            WorkingBlockOption::WorkingBlock(ref block) =>
                String::from(&hex::encode(block.get_block_id())[..6]),
            WorkingBlockOption::TentativeWorkingBlock(ref block_id) =>
                String::from(&hex::encode(block_id)[..5]) + "~",
            _ =>
                String::from("~none~"),
        };

        write!(
            f,
            "({} {} {}, seq {}, wb {}), Node {}{:02}",
            phase, mode, self.view, self.seq_num, wb, ast, self.id,
        )
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum WorkingBlockOption {
    // There is no working block
    NoWorkingBlock,

    // A block has been received in a BlockNew update, but has not been assigned a sequence number
    // yet
    TentativeWorkingBlock(BlockId),

    // There is a current working block
    WorkingBlock(PbftBlock),
}

impl WorkingBlockOption {
    pub fn is_none(&self) -> bool {
        self == &WorkingBlockOption::NoWorkingBlock
    }

    pub fn is_some(&self) -> bool {
        match self {
            &WorkingBlockOption::WorkingBlock(_) => true,
            _ => false,
        }
    }
}

// Information about the PBFT algorithm's state
#[derive(Debug)]
pub struct PbftState {
    // This node's ID
    pub id: u64,

    // The node's current sequence number
    // Always starts at 0; representative of an unknown sequence number.
    pub seq_num: u64,

    // The current view (where the primary's ID is p = v mod network_node_ids.len())
    pub view: u64,

    // Current phase of the algorithm
    pub phase: PbftPhase,

    // Is this node primary or secondary?
    role: PbftNodeRole,

    // Normal operation, view change, or checkpointing. Previous mode is stored when checkpointing
    pub mode: PbftMode,
    pub pre_checkpoint_mode: PbftMode,

    // Map of peers in the network, including ourselves
    network_node_ids: HashMap<u64, PeerId>,

    // The maximum number of faulty nodes in the network
    pub f: u64,

    // Timer used to keep track of whether or not this node has received timely messages from the
    // primary. If a message hasn't been received in a certain amount of time, then this node will
    // initiate a view change.
    pub timeout: Timeout,

    // The current block we're working on
    pub working_block: WorkingBlockOption,
}

impl PbftState {
    pub fn new(id: u64, config: &PbftConfig) -> Self {
        let peer_id_map: HashMap<u64, PeerId> = config
            .peers
            .clone()
            .into_iter()
            .map(|(peer_id, node_id)| (node_id, peer_id))
            .collect();

        // Maximum number of faulty nodes in this network
        let f = ((peer_id_map.len() - 1) / 3) as u64;
        if f == 0 {
            warn!("This network does not contain enough nodes to be fault tolerant");
        }

        PbftState {
            id: id,
            seq_num: 0, // Default to unknown
            view: 0,    // Node ID 0 is default primary
            phase: PbftPhase::NotStarted,
            role: if id == 0 {
                PbftNodeRole::Primary
            } else {
                PbftNodeRole::Secondary
            },
            mode: PbftMode::Normal,
            pre_checkpoint_mode: PbftMode::Normal,
            f: f,
            network_node_ids: peer_id_map,
            timeout: Timeout::new(config.view_change_timeout.clone()),
            working_block: WorkingBlockOption::NoWorkingBlock,
        }
    }

    // Checks to see what type of message we're expecting or sending, based on what phase we're in
    pub fn check_msg_type(&self) -> PbftMessageType {
        match self.phase {
            PbftPhase::PrePreparing => PbftMessageType::PrePrepare,
            PbftPhase::Preparing => PbftMessageType::Prepare,
            PbftPhase::Checking => PbftMessageType::Prepare,
            PbftPhase::Committing => PbftMessageType::Commit,
            _ => PbftMessageType::Unset,
        }
    }

    // Obtain the node ID from a serialized PeerId
    pub fn get_node_id_from_bytes(&self, peer_id: &[u8]) -> Result<u64, PbftError> {
        let deser_id = PeerId::from(peer_id.to_vec());

        let matching_node_ids: Vec<u64> = self.network_node_ids
            .iter()
            .filter(|(_node_id, network_peer_id)| *network_peer_id == &deser_id)
            .map(|(node_id, _network_peer_id)| *node_id)
            .collect();

        if matching_node_ids.len() < 1 {
            Err(PbftError::NodeNotFound)
        } else {
            Ok(matching_node_ids[0])
        }
    }

    pub fn get_own_peer_id(&self) -> PeerId {
        self.network_node_ids[&self.id].clone()
    }

    pub fn get_primary_peer_id(&self) -> PeerId {
        let primary_node_id = self.view % (self.network_node_ids.len() as u64);
        self.network_node_ids[&primary_node_id].clone()
    }

    // Tell if this node is currently a primary
    pub fn is_primary(&self) -> bool {
        self.role == PbftNodeRole::Primary
    }

    // Upgrade this node to primary
    pub fn upgrade_role(&mut self) {
        self.role = PbftNodeRole::Primary;
    }

    // Downgrade this node to secondary
    pub fn downgrade_role(&mut self) {
        self.role = PbftNodeRole::Secondary;
    }

    // Go to a phase and return new phase, if successfully changed
    pub fn switch_phase(&mut self, desired_phase: PbftPhase) -> Option<PbftPhase> {
        let next = match self.phase {
            PbftPhase::NotStarted => PbftPhase::PrePreparing,
            PbftPhase::PrePreparing => PbftPhase::Preparing,
            PbftPhase::Preparing => PbftPhase::Checking,
            PbftPhase::Checking => PbftPhase::Committing,
            PbftPhase::Committing => PbftPhase::Finished,
            PbftPhase::Finished => PbftPhase::NotStarted,
        };
        if desired_phase == next {
            debug!("{}: Changing to {:?}", self, desired_phase);
            self.phase = desired_phase.clone();
            Some(desired_phase)
        } else {
            debug!("{}: Didn't change to {:?}", self, desired_phase);
            None
        }
    }
}
