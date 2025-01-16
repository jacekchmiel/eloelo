import ChevronLeftIcon from "@mui/icons-material/ChevronLeft";
import ChevronRightIcon from "@mui/icons-material/ChevronRight";
import DeleteIcon from "@mui/icons-material/Delete";
import {
	Avatar,
	Box,
	IconButton,
	type IconButtonProps,
	List,
	ListItem,
	ListItemAvatar,
	ListItemText,
	Paper,
	Stack,
	styled,
} from "@mui/material";
import { PresentInLobbyButton } from "./components/PresentInLobbyButton";
import type {
	Avatars,
	Game,
	GameState,
	Player,
	PlayerAvatar,
	Side,
} from "./model";

const Header = styled(Box)(({ theme }) => ({
	...theme.typography.h6,
	textAlign: "left",
	color: theme.palette.text.primary,
}));

const SubHeader = styled(Box)(({ theme }) => ({
	...theme.typography.subtitle1,
	textAlign: "left",
	color: theme.palette.text.primary,
}));

function MoveButton({
	side,
	playerKey,
	...props
}: { side: Side; playerKey: string } & IconButtonProps) {
	return (
		<IconButton
			{...props}
			edge={side === "left" ? "end" : "start"}
			aria-label="delete"
			onClick={async () => {
				// await invoke("move_player_to_other_team", { id: playerKey })
				throw new Error("Not Implemented");
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
		<IconButton
			{...props}
			edge={side === "left" ? "start" : "end"}
			aria-label="delete"
			onClick={async () => {
				// await invoke("remove_player_from_team", { id: playerKey })
				throw new Error("Not Implemented");
			}}
		>
			<DeleteIcon />
		</IconButton>
	);
}

function PlayerProfile({
	player,
	avatarUrl,
}: { player: Player; avatarUrl: string | undefined }) {
	return (
		<>
			<ListItemAvatar>
				<Avatar src={avatarUrl} />
			</ListItemAvatar>
			<ListItemText primary={player.name} secondary={player.elo} />
		</>
	);
}

function RosterRow({
	player,
	side,
	assemblingTeams,
	avatarUrl,
}: {
	player: Player;
	side: Side;
	assemblingTeams: boolean;
	avatarUrl: string | undefined;
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
				disabled={!assemblingTeams}
			/>
			<PlayerProfile {...{ player }} avatarUrl={avatarUrl} />
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

function TeamRoster({
	name,
	players,
	side,
	assemblingTeams,
	avatars,
}: {
	name: string;
	players: Player[];
	side: Side;
	assemblingTeams: boolean;
	avatars: Avatars;
}) {
	const eloSum = players.map((p) => p.elo).reduce((s, v) => s + v, 0);
	return (
		<Paper sx={{ width: "100%", maxWidth: "500px" }}>
			<Stack sx={{ p: 2 }}>
				<Header>{name}</Header>
				<SubHeader>{eloSum.toFixed(0)}</SubHeader>
				<List>
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
};

export function TeamSelector({
	leftPlayers,
	rightPlayers,
	selectedGame,
	availableGames,
	gameState,
	avatars,
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

	return (
		<Stack direction="row" spacing={2} justifyContent="center">
			<TeamRoster
				name={leftTeam}
				players={leftPlayers}
				side="left"
				assemblingTeams={gameState === "assemblingTeams"}
				avatars={avatars}
			/>
			<TeamRoster
				name={rightTeam}
				players={rightPlayers}
				side="right"
				assemblingTeams={gameState === "assemblingTeams"}
				avatars={avatars}
			/>
		</Stack>
	);
}
