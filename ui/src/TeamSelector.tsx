import ChevronLeftIcon from "@mui/icons-material/ChevronLeft";
import ChevronRightIcon from "@mui/icons-material/ChevronRight";
import DeleteIcon from "@mui/icons-material/Delete";
import {
  Avatar,
  IconButton,
  type IconButtonProps,
  List,
  ListItem,
  ListItemAvatar,
  ListItemText,
  Paper,
  Stack,
  Typography,
} from "@mui/material";
import { invoke } from "./Api";
import { CallPlayerButton } from "./components/CallPlayerButton";
import { DefaultTooltip } from "./components/DefaultTooltip";
import { PresentInLobbyButton } from "./components/PresentInLobbyButton";
import type {
  Avatars,
  Game,
  GameState,
  PityBonus,
  Player,
  PlayerAvatar,
  Side,
  TeamPityBonus,
} from "./model";

function MoveButton({
  side,
  playerKey,
  ...props
}: { side: Side; playerKey: string } & IconButtonProps) {
  return (
    <IconButton
      {...props}
      edge={side === "left" ? "end" : "start"}
      onClick={async () => {
        await invoke("move_player_to_other_team", { id: playerKey });
      }}
    >
      {side === "left" ? <ChevronRightIcon /> : <ChevronLeftIcon />}
    </IconButton>
  );
}

function DeleteButton({
  side,
  playerKey,
  ...props
}: { side: Side; playerKey: string } & IconButtonProps) {
  return (
    <DefaultTooltip title="Remove from team">
      <IconButton
        {...props}
        edge={side === "left" ? "start" : "end"}
        onClick={async () => {
          await invoke("remove_player_from_team", { id: playerKey });
        }}
      >
        <DeleteIcon />
      </IconButton>
    </DefaultTooltip>
  );
}

function PlayerProfile({
  player,
  avatarUrl,
  side,
  crown,
}: {
  player: Player;
  avatarUrl: string | undefined;
  side: Side;
  crown: boolean;
}) {
  const textSx = { textAlign: side === "left" ? "start" : "end" };
  const avatarSx = {
    display: "flex",
    justifyContent: side === "left" ? "flex-start" : "flex-end",
  };
  return (
    <>
      <ListItemAvatar sx={avatarSx}>
        <Avatar src={avatarUrl} />
      </ListItemAvatar>
      <ListItemText
        primary={`${crown ? "👑 " : ""}${player.name}`}
        secondary={player.elo}
        sx={textSx}
      />
      {player.loseStreak != null && (
        <StreakIndicator value={-player.loseStreak} />
      )}
    </>
  );
}

function StreakIndicator({ value }: { value: number }) {
  const isLoseStreak = value < 0;

  return (
    <>
      {value !== 0 && (
        <DefaultTooltip title={isLoseStreak ? "Lose Streak" : "Win Streak"}>
          <ListItemText
            primaryTypographyProps={{ color: "error" }}
            sx={{ marginX: 1, flexGrow: 0, minWidth: 24 }}
          >
            {isLoseStreak ? `▼${value}` : `▲${value}`}
          </ListItemText>
        </DefaultTooltip>
      )}
    </>
  );
}

function RosterRow({
  player,
  side,
  assemblingTeams,
  avatarUrl,
  crown,
}: {
  player: Player;
  side: Side;
  assemblingTeams: boolean;
  avatarUrl: string | undefined;
  crown: boolean;
}) {
  return (
    <ListItem
      sx={{
        flexDirection: side === "left" ? "row" : "row-reverse",
        p: 0,
      }}
    >
      <DeleteButton
        side={side}
        playerKey={player.id}
        disabled={!assemblingTeams}
      />
      <PresentInLobbyButton
        side={side}
        playerKey={player.id}
        present={player.presentInLobby}
      />
      <CallPlayerButton side={side} playerKey={player.id} />
      <PlayerProfile
        {...{ player }}
        avatarUrl={avatarUrl}
        side={side}
        crown={crown}
      />
      <MoveButton
        side={side}
        playerKey={player.id}
        disabled={!assemblingTeams}
      />
    </ListItem>
  );
}

const cmp = (a: number, b: number): number => {
  if (a > b) {
    return 1;
  }
  if (a < b) {
    return -1;
  }
  return 0;
};

const diffFormatter = new Intl.NumberFormat(undefined, {
  // This option forces the '+' sign for positive numbers
  signDisplay: "always",
  // These ensure only an integer is displayed
  maximumFractionDigits: 0,
  minimumFractionDigits: 0,
});

function WinChanceText({ winChance }: { winChance?: number }) {
  if (winChance == null) {
    return <Typography>??%</Typography>;
  }
  let winChanceColor = "success.main";
  if (winChance < 0.499) {
    winChanceColor = "error.main";
  } else if (winChance > 0.501) {
    winChanceColor = "success.main";
  }
  const winChanceText = `${(winChance * 100).toFixed(1)}%`;

  return (
    <List sx={{ p: 0, flexGrow: 1 }}>
      <ListItem sx={{ py: 0 }}>
        <ListItemText
          sx={{ my: 0 }}
          primaryTypographyProps={{
            fontSize: 20,
            color: winChanceColor,
            align: "right",
          }}
          primary={winChanceText}
        />
      </ListItem>
    </List>
  );
}

function eloScoreText(elo: number, eloDiff: number) {
  let color = "info.main";
  if (eloDiff > 0) {
    color = "success.main";
  } else if (eloDiff < 0) {
    color = "error.main";
  }
  return (
    <>
      <Typography
        component="span"
        sx={{ mr: 1 }}
      >{`ELO ${elo.toFixed(0)}`}</Typography>
      <Typography fontSize={14} component="span" sx={{ color }}>
        {diffFormatter.format(eloDiff)}
      </Typography>
    </>
  );
}

function mulPityBonus(b: TeamPityBonus | undefined): number {
  const mul = b?.pityBonusMul;
  return mul !== undefined ? mul : 0;
}

function addPityBonus(b: TeamPityBonus | undefined): number {
  const add = b?.pityBonusAdd;
  return add !== undefined ? add : 0;
}

function TeamRoster({
  name,
  players,
  side,
  assemblingTeams,
  avatars,
  pityBonus,
  maxLoseStreak,
  winChance,
  eloDiff,
}: {
  name: string;
  players: Player[];
  side: Side;
  assemblingTeams: boolean;
  avatars: Avatars;
  pityBonus: TeamPityBonus | undefined;
  maxLoseStreak: number;
  winChance?: number;
  eloDiff: number;
}) {
  const eloSum = players.map((p) => p.elo).reduce((s, v) => s + v, 0);
  if (pityBonus && eloSum !== pityBonus.realElo) {
    console.error(
      `eloSum and pityBonus.realElo differ: ${{ eloSum: eloSum, realElo: pityBonus.realElo }}`,
    );
  }
  const pityElo = pityBonus?.pityElo;
  const bonusMulPercent = pityBonus && (pityBonus.pityBonusMul * 100).toFixed();
  const bonusAdd = pityBonus?.pityBonusAdd;
  const showWinChance = winChance != null;

  let pityDescr = "No pity bonus";
  if (bonusMulPercent !== "0" && bonusAdd) {
    pityDescr = `${pityElo} with ${bonusMulPercent}% and ${diffFormatter.format(bonusAdd)} pity bonus`;
  } else if (bonusMulPercent !== "0") {
    pityDescr = `${pityElo} with ${bonusMulPercent}% pity bonus`;
  } else if (bonusAdd) {
    pityDescr = `${pityElo} with ${diffFormatter.format(bonusAdd)} pity bonus`;
  }

  return (
    <Paper sx={{ width: "100%", maxWidth: "500px" }}>
      <Stack sx={{ p: 2 }}>
        <Stack direction="row" sx={{ p: 0 }}>
          <List sx={{ p: 0, flexGrow: 1 }}>
            <ListItem sx={{ py: 0 }}>
              <ListItemText
                sx={{ my: 0 }}
                primaryTypographyProps={{ fontSize: 20 }}
                primary={name}
              />
            </ListItem>
            <ListItem sx={{ pt: 0 }}>
              <ListItemText
                sx={{ mt: 0, ml: 2 }}
                primary={eloScoreText(eloSum, eloDiff)}
                secondary={pityDescr}
              />
            </ListItem>
          </List>
          {showWinChance && <WinChanceText winChance={winChance} />}
        </Stack>
        <List sx={{ p: 0 }}>
          {players
            .sort((a, b) => cmp(a.elo, b.elo) * -1)
            .map((player) => {
              const avatarUrl = avatars.find(
                (a: PlayerAvatar) => a.username === player.discordUsername,
              )?.avatarUrl;
              return (
                <RosterRow
                  {...{ player, side, assemblingTeams }}
                  key={player.name}
                  avatarUrl={avatarUrl}
                  crown={player.loseStreak === maxLoseStreak}
                />
              );
            })}
        </List>
      </Stack>
    </Paper>
  );
}

type TeamSelectorProps = {
  leftPlayers: Player[];
  rightPlayers: Player[];
  selectedGame: string;
  availableGames: Game[];
  gameState: GameState;
  avatars: Avatars;
  pityBonus?: PityBonus;
  winPrediction?: number;
};

export function TeamSelector({
  leftPlayers,
  rightPlayers,
  selectedGame,
  availableGames,
  gameState,
  avatars,
  pityBonus,
  winPrediction,
}: TeamSelectorProps) {
  const selectedGameData = availableGames.find(
    (v: Game): boolean => v.name === selectedGame,
  );
  const leftTeam =
    typeof selectedGameData === "undefined"
      ? "Left team"
      : selectedGameData.leftTeam;
  const rightTeam =
    typeof selectedGameData === "undefined"
      ? "Right team"
      : selectedGameData.rightTeam;
  const maxLoseStreak = Math.max(
    ...(leftPlayers
      .concat(rightPlayers)
      .map((p: Player) => {
        return p.loseStreak;
      })
      .filter(Boolean) as number[]),
  );

  let rightTeamWinChance = undefined;
  if (winPrediction != null) {
    rightTeamWinChance = 1 - winPrediction;
  }
  const leftElo = leftPlayers.map((p) => p.elo).reduce((s, v) => s + v, 0);
  const rightElo = rightPlayers.map((p) => p.elo).reduce((s, v) => s + v, 0);
  const eloDiff = leftElo - rightElo;

  return (
    <Stack direction="row" spacing={2} justifyContent="center">
      <TeamRoster
        name={leftTeam}
        players={leftPlayers}
        side="left"
        assemblingTeams={gameState === "assemblingTeams"}
        avatars={avatars}
        pityBonus={pityBonus?.left}
        maxLoseStreak={maxLoseStreak}
        winChance={winPrediction}
        eloDiff={eloDiff}
      />
      <TeamRoster
        name={rightTeam}
        players={rightPlayers}
        side="right"
        assemblingTeams={gameState === "assemblingTeams"}
        avatars={avatars}
        pityBonus={pityBonus?.right}
        maxLoseStreak={maxLoseStreak}
        winChance={rightTeamWinChance}
        eloDiff={-eloDiff}
      />
    </Stack>
  );
}
