export type Player = {
	name: string;
	elo: number;
};

export type GameState = "assemblingTeams" | "matchInProgress";

export type Team = "left" | "right";

export type Game = {
	name: string;
	leftTeam: string;
	rightTeam: string;
};

export type HistoryEntry = {
	timestamp: Date;
	winner: string[];
	loser: string[];
};

export type History = {
	entries: { [key: string]: HistoryEntry[] };
};

export type EloEloState = {
	availableGames: Game[];
	selectedGame: string;
	leftPlayers: Player[];
	rightPlayers: Player[];
	reservePlayers: Player[];
	gameState: GameState;
	history: History;
};

export type PlayerAvatar = { player: string; avatarUrl: string };
export type Avatars = PlayerAvatar[];
