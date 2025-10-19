import CampaignIcon from "@mui/icons-material/Campaign";
import CancelIcon from "@mui/icons-material/Cancel";
import EmojiEventsIcon from "@mui/icons-material/EmojiEvents";
import PersonIcon from "@mui/icons-material/Person";
import PersonOffIcon from "@mui/icons-material/PersonOff";
import RocketLaunchIcon from "@mui/icons-material/RocketLaunch";
import ShuffleIcon from "@mui/icons-material/Shuffle";
import {
  Box,
  Button,
  ButtonGroup,
  CssBaseline,
  Grid,
  Stack,
  Typography,
} from "@mui/material";
import { grey } from "@mui/material/colors";
import { ThemeProvider, createTheme, styled } from "@mui/material/styles";
import React, {} from "react";
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
import { SplitButton } from "./components/SplitButton";
import { ColorModeContext } from "./components/ThemeSwitcher";
import {
  type DiscordPlayerInfo,
  type EloEloState,
  type GameState,
  type Team,
  extractAvatars,
} from "./model";
import { useColorMode } from "./useColorMode";
import {
  type GenericOptions,
  OptionsView,
  makeGenericOptions,
} from "./views/OptionsView";

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
      {state.gameState === "matchInProgress" && (
        <FightText
          variant="h3"
          color="error"
          sx={{ position: "absolute", top: ".25%", left: "45%" }}
        >
          Fight!
        </FightText>
      )}
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
          onSave={async () => {
            setShowSettings(false);
            await invoke("options", optionValues);
          }}
          onCancel={() => {
            setOptionValues(makeGenericOptions(state.options));
            setShowSettings(false);
          }}
        />
      </DefaultModal>
    </Stack>
  );
}

function StartMatchSplitButton({
  onStartMatch,
  onAddFake,
}: { onStartMatch: () => void; onAddFake: () => void }) {
  const addFakeButton = (
    <Button
      variant="contained"
      color="error"
      key="add-fake"
      onClick={onAddFake}
    >
      Add Fake
    </Button>
  );
  return (
    <SplitButton
      label={"Start Match"}
      endIcon={<RocketLaunchIcon />}
      onClick={onStartMatch}
    >
      {addFakeButton}
    </SplitButton>
  );
}

function AssemblingTeamsActions({
  onShuffleTeams,
  onStartMatch,
  onAddFake,
}: {
  onStartMatch: () => void;
  onAddFake: () => void;
  onShuffleTeams: () => void;
}) {
  return (
    <Grid container>
      <Grid item xs={3}>
        <LobbyCallSplitButton />
      </Grid>

      <Grid item xs={6}>
        <Stack direction="row" justifyContent={"center"}>
          <Button
            onClick={onShuffleTeams}
            variant={"outlined"}
            startIcon={<ShuffleIcon />}
          >
            Shuffle Teams
          </Button>
        </Stack>
      </Grid>

      <Grid item xs={3}>
        <Stack direction="row" justifyContent={"right"}>
          <StartMatchSplitButton {...{ onStartMatch, onAddFake }} />
        </Stack>
      </Grid>
    </Grid>
  );
}

function MatchInProgressActions({
  onFinishMatch,
  onCancelMatch,
}: { onFinishMatch: (winner: Team) => void; onCancelMatch: () => void }) {
  return (
    <Grid container>
      <Grid item xs={3}>
        <LobbyCallSplitButton />
      </Grid>
      <Grid item xs={6}>
        <Stack direction="row" justifyContent="center">
          <ButtonGroup variant="contained">
            <Button
              startIcon={<EmojiEventsIcon />}
              onClick={() => onFinishMatch("left")}
            >
              Left Team Won
            </Button>
            <Button
              endIcon={<EmojiEventsIcon />}
              onClick={() => onFinishMatch("right")}
            >
              Right Team Won
            </Button>
          </ButtonGroup>
        </Stack>
      </Grid>
      <Grid item xs={3}>
        <Stack direction="row" justifyContent="right">
          <Button
            color="error"
            onClick={onCancelMatch}
            variant="contained"
            endIcon={<CancelIcon />}
          >
            Cancel
          </Button>
        </Stack>
      </Grid>
    </Grid>
  );
}

function MatchActionCluster({
  gameState,
  onStartMatch,
  onAddFake,
  onShuffleTeams,
  onFinishMatch,
  onCancelMatch,
}: {
  gameState: GameState;
  onStartMatch: () => void;
  onAddFake: () => void;
  onShuffleTeams: () => void;
  onFinishMatch: (winner: Team) => void;
  onCancelMatch: () => void;
}) {
  return (
    <>
      {gameState === "assemblingTeams" && (
        <AssemblingTeamsActions
          {...{ onStartMatch, onAddFake, onShuffleTeams }}
        />
      )}
      {gameState === "matchInProgress" && (
        <MatchInProgressActions {...{ onFinishMatch, onCancelMatch }} />
      )}
    </>
  );
}

function LobbyCallSplitButton() {
  const clearButton = (
    <Button
      endIcon={<PersonOffIcon color="error" />}
      onClick={async () => {
        await invoke("clear_lobby", {});
      }}
    >
      Clear Lobby
    </Button>
  );
  const fillButton = (
    <Button
      endIcon={<PersonIcon color="success" />}
      onClick={async () => {
        await invoke("fill_lobby", {});
      }}
    >
      Fill Lobby
    </Button>
  );
  return (
    <SplitButton
      label={"Call Lobby"}
      endIcon={<CampaignIcon />}
      onClick={async () => {
        await invoke("call_to_lobby", {});
      }}
    >
      {clearButton}
      {fillButton}
    </SplitButton>
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
      <MatchActionCluster
        gameState={state.gameState}
        onShuffleTeams={async () => {
          await invoke("shuffle_teams", {});
        }}
        onStartMatch={async () => {
          await invoke("start_match", {});
          setStartTimestamp(new Date());
        }}
        onAddFake={() =>
          setFinishMatchModalState({
            fake: true,
            show: true,
            duration: "45m",
          })
        }
        onCancelMatch={async () => await invoke("finish_match", {})}
        onFinishMatch={(winner: Team) => {
          setFinishMatchModalState({
            winner,
            show: true,
            duration: elapsedString(startTimestamp, new Date()),
          });
        }}
      />

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
