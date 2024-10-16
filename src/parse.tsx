import type {
	HistoryEntry,
	History,
	Game,
	Player,
	GameState,
	EloEloState,
} from "./model";

type HistoryEntryTransport = {
	winner: string[];
	loser: string[];
	timestamp: string;
};

type HistoryTransport = {
	entries: { [key: string]: HistoryEntryTransport[] };
};

export type EloEloStateTransport = {
	availableGames: Game[];
	selectedGame: string;
	leftPlayers: Player[];
	rightPlayers: Player[];
	reservePlayers: Player[];
	gameState: GameState;
	history: HistoryTransport;
};

function parseHistoryEntry(historyEntry: HistoryEntryTransport): HistoryEntry {
	const { timestamp, ...entry } = historyEntry;
	return { timestamp: new Date(timestamp), ...entry };
}

function parseHistoryForSingleGame(
	history: HistoryEntryTransport[],
): HistoryEntry[] {
	return history.map(parseHistoryEntry);
}

function parseHistory(historyTransport: HistoryTransport): History {
	const entries = Object.fromEntries(
		Object.entries(historyTransport.entries).map(([k, v]) => [
			k,
			parseHistoryForSingleGame(v),
		]),
	);
	return { entries };
}

export function parseEloEloState(
	eloEloState: EloEloStateTransport,
): EloEloState {
	const { history, ...state } = eloEloState;
	return { history: parseHistory(history), ...state };
}
