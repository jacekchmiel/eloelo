import {
	Avatar,
	Paper,
	Stack,
	Table,
	TableBody,
	TableCell,
	TableContainer,
	TableHead,
	TableRow,
	Tooltip,
} from "@mui/material";
import React from "react";
import type { Avatars, HistoryEntry, Player } from "./model";

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
			<Table>
				<TableHead>
					<TableRow>
						<TableCell>Time</TableCell>
						<TableCell>Winner</TableCell>
						<TableCell>Loser</TableCell>
					</TableRow>
				</TableHead>
				<TableBody>
					{history.map((row) => (
						<TableRow key={row.timestamp.toISOString()}>
							<TableCell>{row.timestamp.toLocaleString()}</TableCell>
							<TableCell>
								<TeamCell
									players={getPlayersByIds(row.winner, players)}
									avatars={avatars}
									highlight={highlightState}
									onAvatarClick={onAvatarClick}
								/>
							</TableCell>
							<TableCell>
								<TeamCell
									players={getPlayersByIds(row.loser, players)}
									avatars={avatars}
									highlight={highlightState}
									onAvatarClick={onAvatarClick}
								/>
							</TableCell>
						</TableRow>
					))}
				</TableBody>
			</Table>
		</TableContainer>
	);
}

function TeamCell({
	players,
	avatars,
	highlight,
	onAvatarClick,
}: {
	players: Player[];
	avatars: Avatars;
	highlight: string | undefined;
	onAvatarClick: (player: string) => void;
}) {
	return (
		<Stack direction="row" spacing={1}>
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
