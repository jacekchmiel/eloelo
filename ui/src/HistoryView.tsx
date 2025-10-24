import {
  Avatar,
  Box,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Tooltip,
  Typography,
} from "@mui/material";
import React from "react";
import { elapsedSecondsString } from "./Duration";
import type { Avatars, HistoryEntry, Player } from "./model";

function percentage(v: number): string {
  const formatter = new Intl.NumberFormat(undefined, {
    maximumFractionDigits: 0,
    minimumFractionDigits: 0,
    style: "percent",
  });
  return formatter.format(v);
}

export function HistoryView({
  history,
  players,
  avatars,
}: { history: HistoryEntry[]; players: Player[]; avatars: Avatars }) {
  const [highlightState, setHighlightState] = React.useState<
    string | undefined
  >(undefined);

  const onAvatarClick = (player: string) => {
    setHighlightState((current) => (current === player ? undefined : player));
  };

  const getPlayersByIds = (ids: string[], players: Player[]): Player[] => {
    return ids.map((id) => {
      const player = players.find((p) => p.id === id);
      return player !== undefined
        ? player
        : {
            id,
            name: id,
            elo: 0,
            discordUsername: undefined,
            presentInLobby: false,
            loseStreak: 0,
          };
    });
  };

  return (
    <TableContainer component={Paper}>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell align="center">Match Time</TableCell>
            <TableCell align="center">Scale</TableCell>
            <TableCell align="right">Duration</TableCell>
            <TableCell align="right">Adv.</TableCell>
            <TableCell />
            <TableCell align="right">Winner</TableCell>
            <TableCell align="left">Loser</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {history.map((row) => (
            <TableRow
              key={row.entry.timestamp.toISOString()}
              sx={{ "&:last-child td, &:last-child th": { border: 0 } }}
            >
              <TableCell align="center" component="th">
                {matchTimeCell(row)}
              </TableCell>
              <TableCell align="center">{winScaleCell(row)}</TableCell>
              <TableCell align="right">{durationCell(row)}</TableCell>
              <TableCell align="right">{winnerChanceCell(row)}</TableCell>
              <TableCell align="center">{fakeIndicatorCell(row)}</TableCell>
              <TableCell>{teamCell(row, "winner")}</TableCell>
              <TableCell>{teamCell(row, "loser")}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableContainer>
  );

  function fakeIndicatorCell(row: HistoryEntry): React.ReactNode {
    return (
      row.entry.fake && (
        <Typography sx={{ fontWeight: "bold", color: "error.main" }}>
          FAKE
        </Typography>
      )
    );
  }

  function teamCell(row: HistoryEntry, side: "winner" | "loser") {
    const tooltip = side === "winner" ? "Winner ELO" : "Loser ELO";
    const elo =
      side === "winner" ? row.metadata.winnerElo : row.metadata.loserElo;
    const justifyContent = side === "winner" ? "right" : "left";
    const playerIds = side === "winner" ? row.entry.winner : row.entry.loser;
    return (
      <Stack>
        <TeamCell
          players={getPlayersByIds(playerIds, players)}
          avatars={avatars}
          highlight={highlightState}
          justifyContent={justifyContent}
          onAvatarClick={onAvatarClick}
        />
        <Stack direction="row" justifyContent={justifyContent}>
          <Tooltip title={tooltip}>
            <Typography sx={{ fontSize: 12, px: 1, pt: 1 }}>{elo}</Typography>
          </Tooltip>
        </Stack>
      </Stack>
    );
  }

  function winnerChanceCell(row: HistoryEntry) {
    const color =
      row.metadata.winnerChance >= 0.49 ? "success.main" : "error.main";
    return (
      <Tooltip title="Expected winner win chance">
        <Typography sx={{ color }}>
          {percentage(row.metadata.winnerChance)}
        </Typography>
      </Tooltip>
    );
  }

  function matchTimeCell(row: HistoryEntry): React.ReactNode {
    return row.entry.timestamp.toLocaleString();
  }

  function durationCell(row: HistoryEntry) {
    return (
      <Tooltip title="Duration">
        <Typography>{elapsedSecondsString(row.entry.duration)}</Typography>
      </Tooltip>
    );
  }

  function winScaleCell(row: HistoryEntry) {
    return (
      <Tooltip title="Win Scale">
        <Typography
          sx={{
            color:
              row.entry.scale === "pwnage"
                ? "error.dark"
                : row.entry.scale === "advantage"
                  ? "warning.dark"
                  : "info.dark",
          }}
        >
          {row.entry.scale}
        </Typography>
      </Tooltip>
    );
  }
}

function TeamCell({
  players,
  avatars,
  highlight,
  justifyContent,
  onAvatarClick,
}: {
  players: Player[];
  avatars: Avatars;
  highlight: string | undefined;
  justifyContent: "left" | "right" | "center";
  onAvatarClick: (player: string) => void;
}) {
  return (
    <Stack direction="row" spacing={1} justifyContent={justifyContent}>
      {players.map((player) => (
        <AvatarWithFallback
          key={player.id}
          dim={highlight !== undefined && highlight !== player.id}
          {...{ avatars, player, onAvatarClick }}
        />
      ))}
    </Stack>
  );
}

function AvatarWithFallback({
  avatars,
  player,
  dim,
  onAvatarClick,
}: {
  avatars: Avatars;
  player: Player;
  dim: boolean;
  onAvatarClick: (player: string) => void;
}) {
  const avatar = avatars.find((a) => a.username === player.discordUsername);
  const sx = dim ? { filter: "grayscale(1) opacity(20%)" } : {};

  return (
    <Tooltip title={player.name}>
      {avatar !== undefined ? (
        <Avatar
          alt={player.name}
          src={avatar.avatarUrl}
          onClick={() => onAvatarClick(player.id)}
          sx={sx}
        />
      ) : (
        <Avatar sx={sx} onClick={() => onAvatarClick(player.id)}>
          {player.name[0]}
        </Avatar>
      )}
    </Tooltip>
  );
}
