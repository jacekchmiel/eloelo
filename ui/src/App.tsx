import EventNoteIcon from "@mui/icons-material/EventNote";

import { PlaylistAddOutlined } from "@mui/icons-material";
import RefreshIcon from "@mui/icons-material/Refresh";
import {
	Box,
	Button,
	CssBaseline,
	FormControl,
	FormControlLabel,
	FormLabel,
	Grid,
	IconButton,
	InputLabel,
	MenuItem,
	Modal,
	Radio,
	RadioGroup,
	Select,
	type SelectChangeEvent,
	Stack,
	TextField,
	Typography,
} from "@mui/material";
import { grey } from "@mui/material/colors";
import { ThemeProvider, createTheme, styled } from "@mui/material/styles";
import React from "react";
import { connectToUiStream, invoke } from "./Api";
import {
	elapsedString,
	isValidDurationString,
	parseDurationString,
	serializeDurationSeconds,
} from "./Duration";
import { HistoryView } from "./HistoryView";
import { ReserveList } from "./ReserveList";
import { TeamSelector } from "./TeamSelector";
import { ColorModeContext, ThemeSwitcher } from "./ThemeSwitcher";
import {
	type DiscordPlayerInfo,
	type EloEloState,
	type WinScale,
	extractAvatars,
} from "./model";
import { type EloEloStateTransport, parseEloEloState } from "./parse";
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

	const menuItems = availableGames.map((game) => (
		<MenuItem value={game} key={game}>
			{game}
		</MenuItem>
	));

	return (
		<Box sx={{ width: "fit-content", minWidth: 120 }}>
			<FormControl fullWidth>
				<InputLabel id="game-select-label">Game</InputLabel>
				<Select
					disabled={disabled}
					labelId="game-select-label"
					id="demo-simple-select"
					value={selectedGame}
					label="Game"
					onChange={handleChange}
					sx={{ backgroundColor: "background.paper" }}
				>
					{menuItems}
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

function EloElo({
	state,
	discordInfo,
}: { state: EloEloState; discordInfo: DiscordPlayerInfo[] }) {
	const [showHistoryState, setShowHistoryState] = React.useState(false);

	return (
		<Stack spacing={2}>
			<Stack
				flexDirection="row"
				justifyContent="space-between"
				alignItems="center"
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
						onClick={async () => setShowHistoryState((prev) => !prev)}
					>
						<EventNoteIcon />
					</IconButton>
					<IconButton onClick={async () => await invoke("refresh_elo", {})}>
						<RefreshIcon />
					</IconButton>
					<ThemeSwitcher />
				</Stack>
			</Stack>
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

type FinishMatchModalState = {
	show: boolean;
	winner?: "left" | "right";
	fake?: boolean;
	duration?: string;
};

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

	const onToggleLobby = async () => {
		const anyoneInLobby =
			state.leftPlayers
				.map((player) => player.presentInLobby)
				.concat(state.rightPlayers.map((p) => p.presentInLobby))
				.filter((p) => p).length > 0;
		if (anyoneInLobby) {
			await invoke("clear_lobby", {});
		} else {
			await invoke("fill_lobby", {});
		}
	};

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

function FinishMatchModal({
	state,
	setState,
}: {
	state: FinishMatchModalState;
	setState: React.Dispatch<React.SetStateAction<FinishMatchModalState>>;
}) {
	const sx = {
		position: "absolute",
		top: "50%",
		left: "50%",
		transform: "translate(-50%, -50%)",
		width: 400,
		bgcolor: "background.paper",
		boxShadow: 24,
		p: 4,
	};

	const userProvidedDurationInvalid = !isValidDurationString(state.duration);
	const showWinnerChoice = state.fake !== undefined && state.fake;
	const buttons: [WinScale, string][] = [
		["pwnage", "Pwnage"],
		["advantage", "Advantage"],
		["even", "Even"],
	];

	const winnerTeam = state.winner === "left" ? "Left Team" : "Right Team";
	const heading = state.fake
		? "Enter fake result"
		: `${winnerTeam} won! How it went?`;
	const noWinner = state.winner === undefined;

	return (
		<Modal open={state.show} onClose={() => setState({ show: false })}>
			<Box sx={sx}>
				<Stack spacing={4}>
					<Typography variant="h6" component="h2">
						{heading}
					</Typography>
					{showWinnerChoice && (
						<FormControl>
							<FormLabel>Winner</FormLabel>
							<RadioGroup
								row
								onChange={(event) => {
									setState((current) => {
										return {
											winner: event.target.value as "left" | "right",
											...current,
										};
									});
								}}
							>
								<FormControlLabel
									value="left"
									control={<Radio />}
									label="Left Team"
								/>
								<FormControlLabel
									value="right"
									control={<Radio />}
									label="Right Team"
								/>
							</RadioGroup>
						</FormControl>
					)}
					<TextField
						label="Duration"
						variant="standard"
						onChange={(event) => {
							setState((current) => {
								return {
									...current,
									duration: event.target.value,
								};
							});
						}}
						error={userProvidedDurationInvalid}
						value={state.duration ?? "0m"}
					/>
					<Stack spacing={2}>
						{buttons.map((b) => {
							const [scale, text] = b;
							return (
								<Button
									key={scale}
									variant="contained"
									onClick={async () => {
										await invoke("finish_match", {
											winner: state.winner,
											scale,
											duration: serializeDurationSeconds(
												parseDurationString(
													state.duration === undefined ? "0" : state.duration,
												),
											),
											fake: state.fake,
										});
										setState({ show: false });
									}}
									disabled={userProvidedDurationInvalid || noWinner}
								>
									{text}
								</Button>
							);
						})}
					</Stack>
				</Stack>
			</Box>
		</Modal>
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
		<Box p={2}>
			<ColorModeContext.Provider value={colorMode}>
				<ThemeProvider theme={theme}>
					<CssBaseline />
					<EloElo state={eloEloState} discordInfo={discordInfoState} />
				</ThemeProvider>
			</ColorModeContext.Provider>
		</Box>
	);
}
