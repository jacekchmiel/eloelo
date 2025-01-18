export type Player = {
	id: string;
	name: string;
	discordUsername: string | undefined;
	elo: number;
	presentInLobby: boolean;
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

export type PlayerAvatar = { username: string; avatarUrl: string | undefined };
export type Avatars = PlayerAvatar[]; // TODO: this type could be removed
export type DiscordPlayerInfo = {
	displayName: string;
	username: string;
	avatarUrl: string | undefined;
};
export type WinScale = "pwnage" | "advantage" | "even";

export function extractAvatars(
	discord_info: DiscordPlayerInfo[],
): PlayerAvatar[] {
	return discord_info.map((e) => {
		return { username: e.username, avatarUrl: e.avatarUrl };
	});
}

export type Side = "left" | "right";
