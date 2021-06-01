use crate::types::{new_player_stats, ResourceAmount};
use crate::{
    game_round::{GameRound, RoundState},
    types::ReputationAmount,
    utils::convert_keys_from_b64,
};
use hdk::prelude::*;
use holo_hash::AgentPubKeyB64;
use std::{collections::HashMap, time::SystemTime};

pub const OWNER_SESSION_TAG: &str = "my_game_sessions";
pub const PARTICIPANT_SESSION_TAG: &str = "game_sessions";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SessionState {
    InProgress,
    Lost { last_round: EntryHash },
    // TODO: when validating things, check that last game round is finished to verify
    // that session itself is finished
    Finished { last_round: EntryHash },
}

#[derive(Clone, Debug, Serialize, Deserialize, Copy)]
pub struct GameParams {
    pub regeneration_factor: f32,
    pub start_amount: ResourceAmount,
    pub num_rounds: u32,
    pub resource_coef: u32,
    pub reputation_coef: u32,
}

#[hdk_entry(id = "game_session", visibility = "public")]
#[derive(Clone)]
pub struct GameSession {
    pub owner: AgentPubKeyB64, // who started the game
    // pub created_at: Timestamp,     // when the game was started
    pub status: SessionState,         // how the game is going
    pub game_params: GameParams,      // what specific game are we playing
    pub players: Vec<AgentPubKeyB64>, // who is playing
}

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub struct GameSessionInput {
    pub game_params: GameParams,
    pub players: Vec<AgentPubKeyB64>,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
pub struct SignalPayload {
    pub game_session: GameSession,
    pub game_session_entry_hash: EntryHash,
    pub previous_round: GameRound,
    pub previous_round_entry_hash: EntryHash,
}

#[hdk_entry(id = "game_scores", visibility = "public")]
#[derive(Clone)]
pub struct GameScores {
    pub game_session: GameSession,
    pub game_session_entry_hash: EntryHash,
    //TODO add the actual results :-)
}

/*
validation rules:

- make sure session is created only when invites are answered and there's at least one accepted
    - TODO: add addresses of accepted invites into game session, later

*/

impl GameSession {
    // called in different contexts:
    // if validation: if round isn't available, validation sin't finished
    // if session state update: round is available
    pub fn update_state(&self, _game_round: GameRound) {
        // this is called every time after GameRound is created

        // if round is lost <= 0:
        //  game session is lost
        // elif number round == num_rounds:
        //  game session is finished
        // else:
        //  game session is in progress
    }
}

// placeholder function that can be called from UI/test, until invitation zoom is added.
pub fn start_dummy_session(player_list: Vec<AgentPubKeyB64>) -> ExternResult<HeaderHash> {
    let input = GameSessionInput {
        game_params: GameParams {
            regeneration_factor: 1.1,
            start_amount: 100,
            num_rounds: 3,
            resource_coef: 3,
            reputation_coef: 2,
        },
        players: player_list,
    };
    new_session(input)
}

/// Create a new GameSession with the confirmed players (who accepted their invites).
/// NOTE: we're only creating session for those who accepted and only if there are at
/// least two of them -- otherwise there won't be any turns.
pub fn new_session(input: GameSessionInput) -> ExternResult<HeaderHash> {
    // agent that starts new game
    let agent_info: AgentInfo = agent_info()?;

    // TODO: get timestamp as systime

    let latest_pubkey = agent_info.agent_latest_pubkey;
    // create entry for game session
    let gs = GameSession {
        owner: AgentPubKeyB64::from(latest_pubkey.clone()),
        status: SessionState::InProgress,
        game_params: input.game_params,
        players: input.players.clone(),
    };
    create_entry(&gs)?;
    let entry_hash_game_session = hash_entry(&gs)?;

    // create link from session owner's address to the game session entry
    create_link(
        latest_pubkey.clone().into(),
        entry_hash_game_session.clone(),
        LinkTag::new(OWNER_SESSION_TAG),
    )?;

    // create links from all game players' addresses to the game session entry
    // NOTE: This block of code causes the following fail during integration tests,
    // right after this fn is executed:
    // ★ Jun 01 15:59:21.759 ERROR holochain::core::workflow::sys_validation_workflow:
    // msg="Direct validation failed" element=Element { signed_header: SignedHeaderHashed
    // { header: HoloHashed(CreateLink(CreateLink { author: AgentPubKey(uhCAkMsIhmhShfPW2csSUGKWy3o3SByrsM5OtjvYw2NUGtWHfx0_a),
    // timestamp: Timestamp(2021-06-01T12:59:21.738976003Z), header_seq: 6,
    // prev_header: HeaderHash(uhCkkHuvSzvfSIW6zacatoWMzTmCZmhN74-q2wqLO19B7xzUUW3PB),
    // base_address: EntryHash(uhCEkflwCaLuNnGSAucS0oTmh5RS6zdpL6VqJDkjwHALr1O23Y3g3),
    // target_address: EntryHash(uhCEkT0HGjuC_XGdaNhNJf76ko6zCMel7oXWwzbqsa9RISus-PP0w),
    // zome_id: ZomeId(0),
    // tag: LinkTag([103, 97, 109, 101, 95, 115, 101, 115, 115, 105, 111, 110, 115]) })),
    // signature: [0, 178, 115, 124, 71, 224, 105, 105, 32, 68, 37, 126, 155, 140, 200, 97, 230, 245, 25, 167, 253, 75, 57, 200, 44, 204, 149, 176, 156, 214, 229, 62, 69, 155, 203, 93, 106, 132, 34, 101, 78, 53, 211, 115, 242, 189, 95, 36, 99, 155, 237, 85, 210, 51, 73, 162, 92, 161, 24, 250, 170, 0, 94, 3] },
    // entry: NotApplicable }
    // ★
    // 15:59:21 [tryorama] error: Test error: {
    //   type: 'error',
    //   data: {
    //     type: 'internal_error',
    //     data: 'Source chain error: InvalidCommit error: The dependency AnyDhtHash(uhCEkflwCaLuNnGSAucS0oTmh5RS6zdpL6VqJDkjwHALr1O23Y3g3) is not held'
    //   }
    // }
    for p in input.players.iter() {
        if AgentPubKey::from(p.clone()) != latest_pubkey {
            create_link(
                AgentPubKey::from(p.clone()).into(),
                entry_hash_game_session.clone(),
                LinkTag::new(PARTICIPANT_SESSION_TAG),
            )?;
        }
    }

    // create game round results for round 0
    // this is starting point for all the game moves of round 1 to reference (implicit link)
    let no_moves: Vec<EntryHash> = vec![];

    // TODO: create a link from session to game round entry to make the round discoverable
    let round_zero = GameRound::new(
        0,
        entry_hash_game_session.clone(),
        RoundState::new(
            // NOTE(e-nastasia): we don't have to do clone() for the start_amount
            // because it's one of the primitive types that implement the Copy trait
            // so it's value will be copied instead of being moved
            gs.game_params.start_amount,
            new_player_stats(input.players.clone()),
        ),
        no_moves,
    );
    let header_hash_round_zero = create_entry(&round_zero)?;
    let entry_hash_round_zero = hash_entry(&round_zero)?;

    // use remote signals from RSM to send a real-time notif to invited players
    //  ! using remote signal to ping other holochain backends, instead of emit_signal
    //  that would talk with the UI
    // NOTE: we're sending signals to notify that a new round has started and
    // that players need to make their moves
    // WARNING: remote_signal is fire and forget, no error if it fails, might be a weak point if this were production happ
    let signal_payload = SignalPayload {
        // tixel: not sure if we need the full objects or only the hashes or both. The tests will tell...
        game_session: gs.clone(),
        game_session_entry_hash: entry_hash_game_session,
        previous_round: round_zero,
        previous_round_entry_hash: entry_hash_round_zero,
    };
    let signal = ExternIO::encode(GameSignal::StartNextRound(signal_payload))?;
    // Since we're storing agent keys as AgentPubKeyB64, and remote_signal only accepts
    // the AgentPubKey type, we need to convert our keys to the expected data type
    remote_signal(signal, convert_keys_from_b64(input.players.clone()))?;
    tracing::debug!("sending signal to {:?}", input.players.clone());

    Ok(header_hash_round_zero)
}

#[derive(Serialize, Deserialize, SerializedBytes, Debug)]
#[serde(tag = "signal_name", content = "signal_payload")]
pub enum GameSignal {
    StartNextRound(SignalPayload),
    GameOver(GameScores),
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixt::prelude::*;
    use hdk::prelude::*;
    use std::vec;

    #[test]
    fn test_new_session() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();
        let game_params = GameParams {
            regeneration_factor: 1.1,
            start_amount: 100,
            num_rounds: 3,
            resource_coef: 3,
            reputation_coef: 2,
        };

        // mock agent info
        let agent_pubkey = fixt!(AgentPubKey);
        let agent_info = AgentInfo::new(agent_pubkey.clone(), agent_pubkey.clone());
        let agent2_pubkey = fixt!(AgentPubKey);

        mock_hdk
            .expect_agent_info()
            .times(1)
            .return_once(move |_| Ok(agent_info));

        // mock create entry
        let headerhash = fixt!(HeaderHash);

        let entryhash = fixt!(EntryHash);
        let closure_header_hash = headerhash.clone();
        mock_hdk
            .expect_create()
            .with(hdk::prelude::mockall::predicate::eq(
                EntryWithDefId::try_from(GameSession {
                    owner: agent_pubkey.clone(),
                    status: SessionState::InProgress,
                    game_params: game_params.clone(),
                    players: vec![agent_pubkey.clone(), agent2_pubkey],
                })
                .unwrap(),
            ))
            .times(1)
            .return_once(move |_| Ok(closure_header_hash));

        let input = GameSessionInput {
            game_params: game_params,
            players: vec![fixt!(AgentPubKey), fixt!(AgentPubKey), fixt!(AgentPubKey)], // 3 random players
        };

        let entry_hash_game_session = fixt!(EntryHash);
        mock_hdk
            .expect_hash_entry()
            .times(1)
            .return_once(move |_| Ok(entry_hash_game_session));

        mock_hdk
            .expect_remote_signal()
            .times(1)
            .return_once(move |_| Ok(()));

        let header_hash_link = fixt!(HeaderHash);
        mock_hdk
            .expect_create_link()
            .times(1)
            .return_once(move |_| Ok(header_hash_link));

        hdk::prelude::set_hdk(mock_hdk);
        new_session(input);
    }
}
