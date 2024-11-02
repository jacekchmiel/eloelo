import EventNoteIcon from "@mui/icons-material/EventNote";

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
	List,
	ListItem,
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
import { type InvokeArgs, invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import React from "react";
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

const invoke = async (event: string, args: InvokeArgs) => {
	console.info({ event, args });
	try {
		await tauriInvoke(event, args);
	} catch (err) {
		console.error(err);
	}
};

function GameSelector({
	selectedGame,
	availableGames,
	disabled,
}: { selectedGame: string; availableGames: string[]; disabled: boolean }) {
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

function EloElo(state: EloEloState) {
	const [discordInfoState, setDiscordInfoState] = React.useState<
		DiscordPlayerInfo[]
	>([]);
	React.useEffect(() => {
		const unlisten = listenToAvatarsEvent();

		return () => {
			unlisten.then((unlisten) => {
				unlisten();
			});
		};
	}, []);

	async function listenToAvatarsEvent() {
		const unlisten = await listen(
			"discord_info",
			(event: { payload: DiscordPlayerInfo[] }) => {
				console.log({ discord_info: event.payload });
				setDiscordInfoState(event.payload);
			},
		);
		return unlisten;
	}

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
					avatars={extractAvatars(discordInfoState)}
					players={state.reservePlayers.concat(
						state.rightPlayers,
						state.leftPlayers,
					)}
				/>
			) : (
				<MainView state={state} discord_info={discordInfoState} />
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
	discord_info,
}: { state: EloEloState; discord_info: DiscordPlayerInfo[] }) {
	const activePlayers = state.leftPlayers
		.concat(state.rightPlayers)
		.concat(state.reservePlayers);
	const playersToAdd = discord_info
		.filter(
			(p) =>
				activePlayers.find((e) => e.discordUsername === p.username) ===
				undefined,
		)
		.sort();
	const avatars = extractAvatars(discord_info);

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
				<List>
					<ListItem>
						<TextField
							label="Duration"
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
					</ListItem>
					{buttons.map((b) => {
						const [scale, text] = b;
						return (
							<ListItem key={scale}>
								<Button
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
							</ListItem>
						);
					})}
				</List>
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

	React.useEffect(() => {
		const unlisten = listenToUiUpdateEvent();

		return () => {
			unlisten.then((unlisten) => {
				unlisten();
			});
		};
	}, []);

	async function listenToUiUpdateEvent() {
		const unlisten = await listen(
			"update_ui",
			(event: { payload: EloEloStateTransport }) => {
				console.log({ state: event.payload });
				const parsed = parseEloEloState(event.payload);
				setEloEloState(parsed);
			},
		);
		return unlisten;
	}

	React.useEffect(() => {
		initializeUi();
	}, []);

	async function initializeUi() {
		await invoke("initialize_ui", {});
	}

	const [mode, setMode] = React.useState<"light" | "dark">("light");
	const colorMode = React.useMemo(
		() => ({
			toggleColorMode: () => {
				setMode((prevMode) => (prevMode === "light" ? "dark" : "light"));
			},
		}),
		[],
	);

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
					<EloElo {...eloEloState} />
				</ThemeProvider>
			</ColorModeContext.Provider>
		</Box>
	);
}
