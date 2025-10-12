import EventNoteIcon from "@mui/icons-material/EventNote";

import RefreshIcon from "@mui/icons-material/Refresh";
import {
  AppBar,
  Box,
  Button,
  CssBaseline,
  FormControl,
  Grid,
  IconButton,
  MenuItem,
  Select,
  type SelectChangeEvent,
  Stack,
  Toolbar,
  Typography,
} from "@mui/material";
import { grey } from "@mui/material/colors";
import { ThemeProvider, createTheme, styled } from "@mui/material/styles";
import React from "react";
import { connectToUiStream, invoke } from "./Api";
import { elapsedString } from "./Duration";
import {
  FinishMatchModal,
  type FinishMatchModalState,
} from "./FinishMatchModal";
import { HistoryView } from "./HistoryView";
import { ReserveList } from "./ReserveList";
import { TeamSelector } from "./TeamSelector";
import { ColorModeContext, ThemeSwitcher } from "./ThemeSwitcher";
import {
  type DiscordPlayerInfo,
  type EloEloState,
  extractAvatars,
} from "./model";
import { useColorMode } from "./useColorMode";

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

function GameSelector({
  selectedGame,
  availableGames,
  disabled,
}: {
  selectedGame: string;
  availableGames: string[];
  disabled: boolean;
}) {
  const handleChange = async (event: SelectChangeEvent<string>) => {
    await invoke("change_game", { name: event.target.value });
  };

  return (
    <Box sx={{ width: "fit-content", minWidth: 120 }}>
      <FormControl fullWidth>
        <Select
          disabled={disabled}
          value={selectedGame}
          onChange={handleChange}
          sx={{ backgroundColor: "background.paper" }}
        >
          {availableGames.map((game) => (
            <MenuItem value={game} key={game}>
              {game}
            </MenuItem>
          ))}
        </Select>
      </FormControl>
    </Box>
  );
}

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

function EloEloHeader({
  state,
  setShowHistoryState,
}: {
  state: EloEloState;
  setShowHistoryState: (mut: (prev: boolean) => boolean) => void;
}) {
  return (
    <AppBar position="static">
      <Toolbar
        sx={{
          paddingY: 1,
          justifyContent: "space-between",
          alignContent: "center",
        }}
      >
        <GameSelector
          availableGames={state.availableGames.map((g) => g.name)}
          selectedGame={state.selectedGame}
          disabled={state.gameState === "matchInProgress"}
        />
        {state.gameState === "matchInProgress" && (
          <FightText variant="h3" color="error">
            Fight!
          </FightText>
        )}
        <Stack flexDirection="row" justifyContent="right">
          <IconButton
            onClick={async () => setShowHistoryState((prev: boolean) => !prev)}
          >
            <EventNoteIcon />
          </IconButton>
          <IconButton onClick={async () => await invoke("refresh_elo", {})}>
            <RefreshIcon />
          </IconButton>
          <ThemeSwitcher />
        </Stack>
      </Toolbar>
    </AppBar>
  );
}

function EloElo({
  state,
  discordInfo,
}: { state: EloEloState; discordInfo: DiscordPlayerInfo[] }) {
  const [showHistoryState, setShowHistoryState] = React.useState(false);

  return (
    <Stack spacing={2} flexGrow={1} maxWidth={1024}>
      <EloEloHeader state={state} setShowHistoryState={setShowHistoryState} />
      {showHistoryState ? (
        <HistoryView
          history={getHistoryForCurrentGame(state)}
          avatars={extractAvatars(discordInfo)}
          players={state.reservePlayers.concat(
            state.rightPlayers,
            state.leftPlayers,
          )}
        />
      ) : (
        <MainView state={state} discordInfo={discordInfo} />
      )}
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
