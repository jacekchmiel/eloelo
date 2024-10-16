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
import type { HistoryEntry, Avatars } from "./model";
import React from "react";

export function HistoryView({
	history,
	avatars,
}: { history: HistoryEntry[]; avatars: Avatars }) {
	const [highlightState, setHighlightState] = React.useState<
		string | undefined
	>(undefined);

	const onAvatarClick = (player: string) => {
		setHighlightState((current) => (current === player ? undefined : player));
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
									players={row.winner}
									avatars={avatars}
									highlight={highlightState}
									onAvatarClick={onAvatarClick}
								/>
							</TableCell>
							<TableCell>
								<TeamCell
									players={row.loser}
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
	players: string[];
	avatars: Avatars;
	highlight: string | undefined;
	onAvatarClick: (player: string) => void;
}) {
	return (
		<Stack direction="row" spacing={1}>
			{players.map((player) => (
				<AvatarWithFallback
					key={player}
					dim={highlight !== undefined && highlight !== player}
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
	player: string;
	dim: boolean;
	onAvatarClick: (player: string) => void;
}) {
	const avatar = avatars.find((a) => a.player === player);
	const sx = dim ? { filter: "grayscale(1) opacity(20%)" } : {};

	return (
		<Tooltip title={player}>
			{avatar !== undefined ? (
				<Avatar
					alt={player}
					src={avatar.avatarUrl}
					onClick={() => onAvatarClick(player)}
					sx={sx}
				/>
			) : (
				<Avatar sx={sx} onClick={() => onAvatarClick(player)}>
					{player[0]}
				</Avatar>
			)}
		</Tooltip>
	);
}
