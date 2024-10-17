import React from "react";
import { InvokeArgs, invoke as tauriInvoke } from "@tauri-apps/api/core";
import {
	Box,
	Button,
	CssBaseline,
	FormControl,
	Grid,
	IconButton,
	Input,
	InputLabel,
	List,
	ListItem,
	MenuItem,
	Modal,
	Select,
	type SelectChangeEvent,
	Stack,
	Typography,
} from "@mui/material";
import { ThemeProvider, createTheme, styled } from "@mui/material/styles";
import { TeamSelector } from "./TeamSelector";
import { ThemeSwitcher, ColorModeContext } from "./ThemeSwitcher";
import { listen } from "@tauri-apps/api/event";
import { ReserveList } from "./ReserveList";
import { grey } from "@mui/material/colors";
import type { Avatars, EloEloState } from "./model";
import RefreshIcon from "@mui/icons-material/Refresh";
import EventNoteIcon from "@mui/icons-material/EventNote";
import { HistoryView } from "./HistoryView";
import { type EloEloStateTransport, parseEloEloState } from "./parse";

const initialAvatarsState: Avatars = [];

const invoke = async (event: string, args: InvokeArgs) => {
	console.log({ event, args });
	//FIXME: should we catch something here?
	await tauriInvoke(event, args);
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
	const [avatarsState, setAvatarsState] = React.useState(initialAvatarsState);
	React.useEffect(() => {
		const unlisten = listenToAvatarsEvent();

		return () => {
			unlisten.then((unlisten) => {
				unlisten();
			});
		};
	}, []);

	async function listenToAvatarsEvent() {
		const unlisten = await listen("avatars", (event: { payload: Avatars }) => {
			console.log({ avatars: event.payload });
			setAvatarsState(event.payload);
		});
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
					<IconButton
						disabled={true}
						onClick={async () => await invoke("refresh_elo", {})}
					>
						<RefreshIcon />
					</IconButton>
					<ThemeSwitcher />
				</Stack>
			</Stack>
			{showHistoryState ? (
				<HistoryView
					history={getHistoryForCurrentGame(state)}
					avatars={avatarsState}
				/>
			) : (
				<MainView state={state} avatars={avatarsState} />
			)}
		</Stack>
	);
}

function MainView({
	state,
	avatars,
}: { state: EloEloState; avatars: Avatars }) {
	const activePlayers = state.leftPlayers
		.concat(state.rightPlayers)
		.concat(state.reservePlayers);
	const playersToAdd = avatars
		.map((a) => a.player)
		.filter((p) => activePlayers.find((e) => e.name === p) === undefined)
		.sort();

	const [finishMatchModalState, setFinishMatchModalState] = React.useState<
		"left" | "right" | undefined
	>(undefined);

	return (
		<>
			<TeamSelector {...state} avatars={avatars} />

			<Grid container>
				{state.gameState === "assemblingTeams" && (
					<>
						<Grid item xs={6}>
							<Stack direction="row" justifyContent="right">
								<Button onClick={async () => await invoke("start_match", {})}>
									Start Match
								</Button>
							</Stack>
						</Grid>
						<Grid item xs={6}>
							<Stack direction="row" justifyContent="left">
								<Button onClick={async () => await invoke("shuffle_teams", {})}>
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
									onClick={() => setFinishMatchModalState("left")}
									// onClick={async () =>
									// 	await invoke("finish_match", { winner: "left" })
									// }
								>
									Left Team Won
								</Button>
							</Stack>
						</Grid>
						<Grid item xs={6}>
							<Stack direction="row" justifyContent="space-between">
								<Button onClick={() => setFinishMatchModalState("right")}>
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
				onClose={() => setFinishMatchModalState(undefined)}
				onProceed={async (winScale, duration) =>
					await invoke("finish_match", {
						winner: finishMatchModalState,
						scale: winScale,
						duration: duration,
					})
				}
			/>
		</>
	);
}

function FinishMatchModal({
	open,
	onClose,
	onProceed,
}: {
	open: boolean;
	onClose: () => void;
	onProceed: (
		winScale: "domination" | "advantage" | "even",
		duration: string,
	) => void;
}) {
	const sx = {
		position: "absolute",
		top: "50%",
		left: "50%",
		transform: "translate(-50%, -50%)",
		width: 400,
		bgcolor: "background.paper",
		// border: "2px solid #000",
		boxShadow: 24,
		p: 4,
	};

	return (
		<Modal open={open} onClose={onClose}>
			<Box sx={sx}>
				<Typography id="modal-modal-title" variant="h6" component="h2">
					How it went?
				</Typography>
				<List>
					<ListItem>Duration</ListItem>
					<ListItem>
						<Button
							onClick={async () => {
								// FIXME: this value is incorrect, but the error message is rather unhelpful
								onProceed("domination", "1h");
								onClose();
							}}
						>
							Dominated
						</Button>
					</ListItem>
					<ListItem>
						<Button
							onClick={async () => {
								onProceed("advantage", "1h");
								onClose();
							}}
						>
							Advantage
						</Button>
					</ListItem>
					<ListItem>
						<Button
							onClick={async () => {
								onProceed("even", "1h");
								onClose();
							}}
						>
							Even
						</Button>
					</ListItem>
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
