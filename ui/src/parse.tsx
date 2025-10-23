import type {
  EloEloState,
  Game,
  GameState,
  History,
  HistoryEntry,
  OptionsGroup,
  PityBonus,
  Player,
  WinScale,
} from "./model";

type HistoryEntryTransport = {
  entry: {
    winner: string[];
    loser: string[];
    timestamp: string;

    duration: number;
    scale: WinScale;
    fake: boolean;
  };
  metadata: {
    winnerElo: number;
    loserElo: number;
    winnerChance: number;
  };
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
  pityBonus: PityBonus | undefined;
  options: OptionsGroup[];
  winPrediction: number;
  shuffleTemperature: number;
};

function parseHistoryEntry(historyEntry: HistoryEntryTransport): HistoryEntry {
  const { entry, metadata } = historyEntry;
  const { timestamp, scale, ...rest } = entry;
  return {
    entry: {
      timestamp: new Date(timestamp),
      scale: scale.toLowerCase() as WinScale,
      ...rest,
    },
    metadata,
  };
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
