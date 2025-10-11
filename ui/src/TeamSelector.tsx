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
	Tooltip,
	styled,
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

const Header = styled(Box)(({ theme }) => ({
	...theme.typography.h5,
	textAlign: "left",
	color: theme.palette.text.primary,
}));

const Info = styled(Box)(({ theme }) => ({
	...theme.typography.subtitle2,
	textAlign: "right",
	color: theme.palette.text.secondary,
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
}: { player: Player; avatarUrl: string | undefined; side: Side }) {
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
			<ListItemText primary={player.name} secondary={player.elo} sx={textSx} />
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
						{isLoseStreak ? `${value}▼` : `${value}▲`}
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
			/>
			<CallPlayerButton side={side} playerKey={player.id} />
			<PlayerProfile {...{ player }} avatarUrl={avatarUrl} side={side} />
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
	pityBonus,
}: {
	name: string;
	players: Player[];
	side: Side;
	assemblingTeams: boolean;
	avatars: Avatars;
	pityBonus: TeamPityBonus | undefined;
}) {
	const eloSum = players.map((p) => p.elo).reduce((s, v) => s + v, 0);
	if (pityBonus && eloSum !== pityBonus.realElo) {
		console.error(
			`eloSum and pityBonus.realElo differ: ${{ eloSum: eloSum, realElo: pityBonus.realElo }}`,
		);
	}
	const pityElo = pityBonus?.pityElo;
	const bonus = pityBonus && ((1 - pityBonus.pityBonus) * 100).toFixed();
	return (
		<Paper sx={{ width: "100%", maxWidth: "500px" }}>
			<Stack sx={{ p: 2 }}>
				<Header>{name}</Header>
				<Stack direction="row" justifyContent="space-between">
					<Info>{eloSum.toFixed(0)}</Info>
					{pityBonus && (
						<Info>{`${pityElo} with pity bonus of -${bonus}%`}</Info>
					)}
				</Stack>
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
	pityBonus: PityBonus | null;
};

export function TeamSelector({
	leftPlayers,
	rightPlayers,
	selectedGame,
	availableGames,
	gameState,
	avatars,
	pityBonus,
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
				pityBonus={pityBonus?.left}
			/>
			<TeamRoster
				name={rightTeam}
				players={rightPlayers}
				side="right"
				assemblingTeams={gameState === "assemblingTeams"}
				avatars={avatars}
				pityBonus={pityBonus?.right}
			/>
		</Stack>
	);
}
