<script>
    import { afterUpdate } from "svelte";

    import GameMove from "./GameMove.svelte";
    import GameResults from "./GameResults.svelte";
    import GameRound from "./GameRound.svelte";
    import { bufferToBase64, shorten, shortenBase64 } from "./utils";

    export let nickname = "";
    export let gamecode = "";
    export let agentPubKeyB64 = "";
    export let action = "GAME_BEGIN";
    export let current_round_hash = "";
    let resources_default_start = 100;
    $: total_resources = calculateTotalTaken(rounds);

    let game_status = "WAITING_PLAYERS"; // "MAKE_MOVE" "WAIT_NEXT_ROUND" "WAIT_GAME_SCORE" "GAME_OVER"
    let result_status = "WAIT_RESULTS"; //"GAME_LOST" "GAME_WON"

    let rounds = [];
    let players = [];
    let player_stats = [];

    function calculateTotalTaken(rounds) {
        if (!rounds) {
            console.error("Rounds array is empty");
        }
        let totalTaken = 0;
        let totalGrown = 0;
        for (let i = 0; i < rounds.length; i++) {
            const round = rounds[i];
            if (round.fake === true) {
                continue;
            }
            let moves = round.moves;
            for (let j = 0; j < moves.length; j++) {
                const move = moves[j];
                totalTaken = totalTaken + move.resourcesTaken;
            }
        }
        return resources_default_start - totalTaken + totalGrown;
    }

    async function refreshPlayerList() {
        const playerProfiles = await window.appClient.getPlayers(gamecode);
        console.log("players", playerProfiles);
        players = playerProfiles;
    }

    async function play() {
        console.log("action", action);
        if (action == "GAME_BEGIN") {
            current_round_hash = await window.appClient.startGame(gamecode);
            console.log("game started", current_round_hash);
            game_status = "MAKE_MOVE";
        } else if (action == "GAME_JOIN") {
            console.log("check if game has been started");
            try {
                let result = await window.appClient
                    .currentRoundForGameCode(gamecode)
                    .catch();
                console.log("result:", result);
                if (result) {
                    // get current round hash
                    console.log("game joined", result);
                    current_round_hash = result;
                    game_status = "MAKE_MOVE";
                } else {
                    alert(
                        "Still waiting on other players or game host to start playing..."
                    );
                }
            } catch (error) {
                if (error.type === "ribosome_error" || error.type === "error") {
                    console.info("Error: ", error.data);
                    alert("Error: " + error.data.data);
                }
            }
        }
    }

    async function makeMove(event) {
        game_status = "WAIT_NEXT_ROUND";
        let resources = event.detail.resources;
        console.log("taking resources:", resources);
        // (amount, prev_round_hash)
        let result = await window.appClient.makeMove(
            resources,
            current_round_hash
        );
        console.log(
            "added move to round with header hash:",
            current_round_hash
        );
        console.log("result make move", result);

        addFakePendingRound(nickname, resources, calculateTotalTaken(rounds)); //TODO
    }

    async function updateRound() {
        if (game_status == "GAME_OVER") {
            return;
        }

        /* get last rounds from dht
        if roundnum equal, then update fake round to actual round
        else do nothing
        */
        if (!current_round_hash || rounds.length === 0) {
            console("no round hash????: ", rounds.length);
            return;
        }
        let latest_game_info = await window.appClient.tryCloseRound(
            current_round_hash
        );
        console.log("current round info:", latest_game_info);
        console.log("rounds:", rounds);
        let last_round = rounds[rounds.length - 1];
        if (latest_game_info.type === "error") {
            console.info("Still waiting? ", latest_game_info.data);
        }

        if (latest_game_info.next_action === "WAITING") {
            return; // currently not needed
        }
        console.log("next action:", latest_game_info.next_action);

        if (last_round.fake) {
            //} && last_round.round_num === latest_game_info.round_num){
            //rounds.pop(); //remove fake round
            console.log("update fake round to real");
            addRealCompletedRound(latest_game_info);
            console.log("set new round hash");
            current_round_hash = latest_game_info.current_round_entry_hash;
        }

        if (latest_game_info.next_action === "SHOW_GAME_RESULTS") {
            game_status = "WAIT_GAME_SCORE";
            calculateResults();
        } else if (latest_game_info.next_action === "START_NEXT_ROUND") {
            game_status = "MAKE_MOVE";
        } else {
            console.error("unknown action:", latest_game_info.next_action);
            alert("Still waiting on other players");
        }
    }

    function addFakePendingRound(nickname, resources_taken, resources_total) {
        let fakePendingRound = {
            round_num: rounds.length + 1,
            resources_left: resources_total,
            current_round_entry_hash: "",
            prev_round_entry_hash: "",
            game_session_hash: "",
            next_action: "WAITING",
            moves: [
                {
                    nickname: nickname,
                    id: "",
                    resourcesTaken: resources_taken,
                },
            ],
            fake: true,
        };

        rounds = [...rounds, fakePendingRound];
        console.log("rounds: ", rounds);
    }

    function addRealCompletedRound(latest_game_info) {
        let last_round = rounds[rounds.length - 1];
        if (last_round.round_num !== latest_game_info.round_num) {
            console.log("last round is different. Oink?");
            return;
        }
        let convertedMoves = [];

        latest_game_info.moves.forEach(convertMove);
        function convertMove(move, index) {
            console.debug("move: ", move);
            let x = {
                nickname: findPlayerNameByHash(move[2]),
                id: move[2],
                resourcesTaken: move[0],
            };
            convertedMoves.push(x);
        }

        last_round.current_round_entry_hash =
            latest_game_info.current_round_entry_hash;
        last_round.prev_round_entry_hash =
            latest_game_info.prev_round_entry_hash;
        last_round.round_num = latest_game_info.round_num;
        last_round.fake = false;
        last_round.moves = convertedMoves;
        last_round.resources_left = calculateTotalTaken(rounds);

        rounds = rounds;
        console.log("rounds: ", rounds);
    }

    function findPlayerNameByHash(hash) {
        for (let i = 0; i < players.length; i++) {
            const p = players[i];
            if (p.player_id.toString() === hash.toString()) {
                return p.nickname;
            }
        }
        return "name not found";
    }

    function calculateResults() {
        let all_moves = [];
        for (let i = 0; i < rounds.length; i++) {
            const round = rounds[i];
            all_moves = all_moves.concat(round.moves);
        }
        let stats = sumMoves(all_moves);
        console.log(stats);
        let calculated_results = {
            total_score: calculateTotalTaken(rounds),
            stats: stats,
        };
        player_stats = calculated_results.stats;
        game_status = "GAME_OVER";
        if (calculated_results.total_score > 0) {
            result_status = "GAME_WON";
        } else {
            result_status = "GAME_LOST";
        }
    }

    function sumMoves(allMoves) {
        let stats = {};
        for (let i = 0; i < allMoves.length; i++) {
            const move = allMoves[i];
            const player = bufferToBase64(move.id);
            if (stats[player] === undefined) {
                stats[player] = 0;
            }
            stats[player] = stats[player] + parseInt(move.resourcesTaken);
        }

        return Object.entries(stats);
    }

    afterUpdate(() => {
        console.log("scrolling to", document.body.scrollHeight);
        window.scrollTo(0, document.body.scrollHeight);
    });
</script>

<section>
    <aside>
        <h3>Your nickname</h3>
        <span id="nickname" style="font-size: 4rem !important;">{nickname}</span
        >
        <i style="color:silver">{shorten(agentPubKeyB64, 10, 20)}</i>
    </aside>
    <aside>
        <h3>Game code</h3>
        <p style="font-size: 4rem !important;" id="gamecode">{gamecode}</p>
    </aside>

    <aside>
        <h3>Players</h3>
        <p style="font-size: 4rem !important;" id="playercount">
            {players.length}
        </p>
    </aside>
</section>

<div class="playerlist">
    {#each players as player, i}
        <button
            ><span class="playername">{player.nickname}</span>
            <br />
            <sup class="hashsup">
                {shortenBase64(player.player_id)}
            </sup>
        </button>
    {/each}
</div>

<div class="gamerounds">
    {#if game_status == "WAITING_PLAYERS"}
        <div class="columncentered">
            <p>
                Wait until all player joined the game.
                <br />
                <a
                    id="refresh_player_list"
                    href="#"
                    on:click={refreshPlayerList}>Refresh</a
                >
                <br />
                {#if players.length != 0}
                    <button
                        id="start_play_btn"
                        class="startgamebutton"
                        on:click={play}>Play!</button
                    >
                {/if}
            </p>
        </div>
    {:else}
        <div class="columncentered">
            <a href="#" on:click={refreshPlayerList}>Refresh player list</a>
        </div>
        <section>
            <aside class="gameround">
                <p>
                    This commons starts with: <strong
                        >{resources_default_start} resources</strong
                    >
                </p>
            </aside>
        </section>
        <!-- TODO for each list of rounds played-->
        {#each rounds as round, i}
            <GameRound
                {round}
                moves={round.moves}
                on:updateRound={updateRound}
            />
        {/each}
        {#if game_status == "MAKE_MOVE"}
            <GameMove on:makeMove={makeMove} total_resource={total_resources} />
        {/if}
        {#if game_status == "WAIT_NEXT_ROUND"}
            <div style="text-align:center;">
                Wait until round is complete...
                <br />
                Click refresh
            </div>
        {/if}

        <!-- ONLY if game ended-->
        {#if game_status == "GAME_OVER"}
            <GameResults
                {player_stats}
                resources_left={total_resources}
                {players}
            />
            {#if result_status == "GAME_LOST"}
                <div style="text-align:center;">
                    <h1>
                        We lost
                        <br />
                        ...what a tragedy
                    </h1>
                    <p>
                        We overextended our commons.<br />We went beyond it
                        natural limits.<br />It imploded :'‑(
                    </p>
                </div>
            {/if}
            {#if result_status == "GAME_WON"}
                <div style="text-align:center;">
                    <h1>Yes we made it... together.</h1>
                    <p>
                        We never took more than <br />what our commons could
                        regrow.<br />There is a future...
                    </p>
                </div>
            {/if}
        {/if}
    {/if}
</div>

<style>
    .gameround {
        width: 70%;
        margin-top: 1rem;
        margin-bottom: 1rem;
    }
    .playername {
        font-size: 2.5rem;
    }
    .hashsup {
        color: #118bee;
        background-color: white;
    }
    .gamerounds {
        display: flex;
        flex-direction: column;
        justify-content: center;
        margin-top: 1rem;
    }

    .columncentered {
        display: flex;
        flex-direction: column;
        justify-content: center;
        text-align: center;
        margin-top: 1rem;
    }

    .columnright {
        display: flex;
        flex-direction: column;
        justify-content: center;
        text-align: right;
        margin-top: 1rem;
    }

    .startgamebutton {
        text-align: center;
        width: 30%;
    }

    .playerlistRefresh {
        display: flex;
        justify-content: center;
        margin-top: 1rem;
    }

    .playerlist {
        padding-left: 10%;
        padding-right: 10%;
        display: flex;
        flex-wrap: wrap;
        justify-content: center;
        margin: 1rem;
    }

    .playerlist button {
        background-color: white;
        color: #118bee;
    }

    .playerlist button + button {
        margin-left: 1rem;
    }
</style>
