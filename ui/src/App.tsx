import {
  Box,
  Button,
  CssBaseline,
  Grid,
  Stack,
  Typography,
} from "@mui/material";
import { grey } from "@mui/material/colors";
import { ThemeProvider, createTheme, styled } from "@mui/material/styles";
import React from "react";
import { connectToUiStream, invoke } from "./Api";
import { EloEloAppBar } from "./AppBar";
import { elapsedString } from "./Duration";
import {
  FinishMatchModal,
  type FinishMatchModalState,
} from "./FinishMatchModal";
import { HistoryView } from "./HistoryView";
import { ReserveList } from "./ReserveList";
import { TeamSelector } from "./TeamSelector";
import { DefaultModal } from "./components/DefaultModal";
import { ColorModeContext } from "./components/ThemeSwitcher";
import {
  type DiscordPlayerInfo,
  type EloEloState,
  extractAvatars,
} from "./model";
import { useColorMode } from "./useColorMode";
import {
  type GenericOptions,
  OptionsView,
  makeGenericOptions,
} from "./views/OptionsView";

// const invoke = async (command: string, args: object) => {
// 	console.info({ command, args });
// 	const url = `${location.href}api/v1/${command}`;
// 	const response = await fetch(url, {
// 		method: "POST",
// 		body: JSON.stringify(args),
// 	});
// 	const body = await response.json();
// 	if (!response.ok) {
// 		const status = response.status;
// 		console.error({ status, body });
// 	}
// };

const FightText = styled(Typography)({
  animation: "shake 0.5s",
  animationIterationCount: "infinite",
  "@keyframes shake": {
    "0%": { transform: "translate(1px, 1px) rotate(0deg);" },
    "10%": { transform: "translate(-1px, -2px) rotate(-1deg);" },
    "20%": { transform: "translate(-3px, 0px) rotate(1deg);" },
    "30%": { transform: "translate(3px, 2px) rotate(0deg);" },
    "40%": { transform: "translate(1px, -1px) rotate(1deg);" },
    "50%": { transform: "translate(-1px, 2px) rotate(-1deg);" },
    "60%": { transform: "translate(-3px, 1px) rotate(0deg);" },
    "70%": { transform: "translate(3px, 1px) rotate(-1deg);" },
    "80%": { transform: "translate(-1px, -1px) rotate(1deg);" },
    "90%": { transform: "translate(1px, 2px) rotate(0deg);" },
    "100%": { transform: "translate(1px, -2px) rotate(-1deg);" },
  },
});

function EloElo({
  state,
  discordInfo,
}: { state: EloEloState; discordInfo: DiscordPlayerInfo[] }) {
  const [showMatchHistory, setShowMatchHistory] = React.useState(false);
  const [showSettings, setShowSettings] = React.useState(false);
  const [optionValues, setOptionValues] = React.useState<GenericOptions>({});

  React.useEffect(() => {
    setOptionValues(makeGenericOptions(state.options));
  }, [state.options]);

  return (
    <Stack spacing={2} flexGrow={1} maxWidth={1024}>
      <EloEloAppBar
        state={state}
        setShowMatchHistory={setShowMatchHistory}
        setShowSettings={setShowSettings}
      />
      <MainView state={state} discordInfo={discordInfo} />
      <DefaultModal
        show={showMatchHistory}
        setShow={setShowMatchHistory}
        size="large"
      >
        <HistoryView
          history={getHistoryForCurrentGame(state)}
          avatars={extractAvatars(discordInfo)}
          players={state.reservePlayers.concat(
            state.rightPlayers,
            state.leftPlayers,
          )}
        />
      </DefaultModal>
      <DefaultModal show={showSettings} setShow={setShowSettings}>
        <OptionsView
          options={state.options}
          values={optionValues}
          setValues={setOptionValues}
        />
      </DefaultModal>
    </Stack>
  );
}

function MainView({
  state,
  discordInfo,
}: {
  state: EloEloState;
  discordInfo: DiscordPlayerInfo[];
}) {
  const activePlayers = state.leftPlayers
    .concat(state.rightPlayers)
    .concat(state.reservePlayers);
  const playersToAdd = discordInfo
    .filter(
      (p) =>
        activePlayers.find((e) => e.discordUsername === p.username) ===
        undefined,
    )
    .sort();
  const avatars = extractAvatars(discordInfo);

  const [finishMatchModalState, setFinishMatchModalState] =
    React.useState<FinishMatchModalState>({ show: false });
  const [startTimestamp, setStartTimestamp] = React.useState<Date>(new Date(0));

  return (
    <>
      <TeamSelector {...state} avatars={avatars} />

      <Grid container>
        {state.gameState === "assemblingTeams" && (
          <>
            <Grid item xs={6}>
              <Stack direction="row" justifyContent="right">
                <Button
                  onClick={async () => {
                    await invoke("start_match", {});
                    setStartTimestamp(new Date());
                  }}
                >
                  Start Match
                </Button>
              </Stack>
            </Grid>
            <Grid item xs={6}>
              <Stack direction="row" justifyContent="left">
                <Button
                  onClick={async () => {
                    await invoke("shuffle_teams", {});
                  }}
                >
                  Shuffle Teams
                </Button>
                <Button
                  color="error"
                  onClick={async () => {
                    setFinishMatchModalState({
                      fake: true,
                      show: true,
                      duration: "45m",
                    });
                  }}
                >
                  Add Fake
                </Button>
              </Stack>
            </Grid>
          </>
        )}
        {state.gameState === "matchInProgress" && (
          <>
            <Grid item xs={6}>
              <Stack direction="row" justifyContent="right">
                <Button
                  onClick={() => {
                    setFinishMatchModalState({
                      winner: "left",
                      show: true,
                      duration: elapsedString(startTimestamp, new Date()),
                    });
                  }}
                >
                  Left Team Won
                </Button>
              </Stack>
            </Grid>
            <Grid item xs={6}>
              <Stack direction="row" justifyContent="space-between">
                <Button
                  onClick={() => {
                    setFinishMatchModalState({
                      winner: "right",
                      show: true,
                      duration: elapsedString(startTimestamp, new Date()),
                    });
                  }}
                >
                  Right Team Won
                </Button>
                <Button
                  color="error"
                  onClick={async () => await invoke("finish_match", {})}
                >
                  Cancel
                </Button>
              </Stack>
            </Grid>
          </>
        )}
      </Grid>
      <Grid item xs={12}>
        <Stack direction="row" justifyContent="center">
          <h3>Lobby</h3>
          <Button
            onClick={async () => {
              await invoke("call_to_lobby", {});
            }}
          >
            Call
          </Button>
          <Button
            onClick={async () => {
              await invoke("clear_lobby", {});
            }}
          >
            Clear
          </Button>
          <Button
            onClick={async () => {
              await invoke("fill_lobby", {});
            }}
          >
            Fill
          </Button>
        </Stack>
      </Grid>
      <ReserveList
        players={state.reservePlayers}
        assemblingTeams={state.gameState === "assemblingTeams"}
        avatars={avatars}
        playersToAdd={playersToAdd}
      />
      <FinishMatchModal
        state={finishMatchModalState}
        setState={setFinishMatchModalState}
      />
    </>
  );
}

function getHistoryForCurrentGame(state: EloEloState) {
  const maybeHistory = state.history.entries[state.selectedGame];
  return maybeHistory === undefined ? [] : maybeHistory;
}

const initialEloEloState: EloEloState = {
  availableGames: [],
  selectedGame: "",
  leftPlayers: [],
  rightPlayers: [],
  reservePlayers: [],
  gameState: "assemblingTeams",
  history: { entries: {} },
  pityBonus: undefined,
  options: [],
};

export default function App() {
  const [eloEloState, setEloEloState] = React.useState(initialEloEloState);
  const [discordInfoState, setDiscordInfoState] = React.useState<
    DiscordPlayerInfo[]
  >([]);
  const { mode, colorMode } = useColorMode();

  React.useEffect(() => {
    const closeConnection = connectToUiStream({
      onError: (error: string) => {
        console.error({ error });
      },
      onUiState: setEloEloState,
      onDiscordInfo: setDiscordInfoState,
    });

    return () => {
      closeConnection.then((close) => {
        close();
      });
    };
  }, []);

  const theme = React.useMemo(
    () =>
      createTheme({
        palette: {
          mode,
          background: {
            default: mode === "light" ? grey[100] : grey[900],
          },
        },
      }),
    [mode],
  );

  return (
    <Box p={2} sx={{ display: "flex", justifyContent: "center" }}>
      <ColorModeContext.Provider value={colorMode}>
        <ThemeProvider theme={theme}>
          <CssBaseline />
          <EloElo state={eloEloState} discordInfo={discordInfoState} />
        </ThemeProvider>
      </ColorModeContext.Provider>
    </Box>
  );
}
