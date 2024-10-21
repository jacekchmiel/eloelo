import EventNoteIcon from "@mui/icons-material/EventNote";
import RefreshIcon from "@mui/icons-material/Refresh";
import {
	Box,
	Button,
	CssBaseline,
	FormControl,
	Grid,
	IconButton,
	InputLabel,
	List,
	ListItem,
	MenuItem,
	Modal,
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
	// biome-ignore lint/nursery/noConsole: important log
	console.log({ event, args });
	try {
		await tauriInvoke(event, args);
	} catch (err) {
		// biome-ignore lint/nursery/noConsole: important log
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
				// biome-ignore lint/nursery/noConsole: important log
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

	const [finishMatchModalState, setFinishMatchModalState] = React.useState<
		"left" | "right" | undefined
	>(undefined);
	const [startTimestamp, setStartTimestamp] = React.useState<Date>(new Date(0));
	const [duration, setDuration] = React.useState("0m");

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
										setDuration(elapsedString(startTimestamp, new Date()));
										setFinishMatchModalState("left");
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
										setDuration(elapsedString(startTimestamp, new Date()));
										setFinishMatchModalState("right");
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
				open={finishMatchModalState !== undefined}
				duration={duration}
				setDuration={setDuration}
				onClose={() => setFinishMatchModalState(undefined)}
				onProceed={async (winScale, durationSeconds) =>
					await invoke("finish_match", {
						winner: finishMatchModalState,
						scale: winScale,
						duration: serializeDurationSeconds(durationSeconds),
					})
				}
			/>
		</>
	);
}

function FinishMatchModal({
	open,
	duration,
	setDuration,
	onClose,
	onProceed,
}: {
	open: boolean;
	duration: string;
	setDuration: (duration: string) => void;
	onClose: () => void;
	onProceed: (winScale: WinScale, durationSeconds: number) => Promise<void>;
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

	const userProvidedDurationInvalid = !isValidDurationString(duration);

	const buttons: [WinScale, string][] = [
		["pwnage", "Pwnage"],
		["advantage", "Advantage"],
		["even", "Even"],
	];

	return (
		<Modal open={open} onClose={onClose}>
			<Box sx={sx}>
				<Typography variant="h6" component="h2">
					How it went?
				</Typography>
				<List>
					<ListItem>
						<TextField
							label="Duration"
							onChange={(event) => {
								setDuration(event.target.value);
							}}
							error={userProvidedDurationInvalid}
							value={duration}
						/>
					</ListItem>
					{buttons.map((b) => {
						const [command, text] = b;
						return (
							<ListItem key={command}>
								<Button
									variant="contained"
									onClick={async () => {
										await onProceed(command, parseDurationString(duration));
										onClose();
									}}
									disabled={userProvidedDurationInvalid}
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
				// biome-ignore lint/nursery/noConsole: important log
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
