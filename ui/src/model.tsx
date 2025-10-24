export type Player = {
  id: string;
  name: string;
  discordUsername: string | undefined;
  elo: number;
  presentInLobby: boolean;
  loseStreak: number | undefined;
};

export type GameState = "assemblingTeams" | "matchInProgress";

export type Team = "left" | "right";

export type Game = {
  name: string;
  leftTeam: string;
  rightTeam: string;
};

export type HistoryEntry = {
  entry: {
    timestamp: Date;
    winner: string[];
    loser: string[];
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

export type History = {
  entries: { [key: string]: HistoryEntry[] };
};

export type OptionType = "integer" | "decimal" | "string" | "boolean";

export type DescribedOption = {
  name: string;
  key: string;
  type: OptionType;
  value: number | string | boolean;
};

export type OptionsGroup = {
  name: string;
  key: string;
  options: DescribedOption[];
};

export type EloEloState = {
  availableGames: Game[];
  selectedGame: string;
  leftPlayers: Player[];
  rightPlayers: Player[];
  reservePlayers: Player[];
  gameState: GameState;
  history: History;
  pityBonus: PityBonus | undefined;
  options: OptionsGroup[];
  winPrediction?: number;
  shuffleTemperature: number;
};

export type PityBonus = {
  left: TeamPityBonus;
  right: TeamPityBonus;
};

export type TeamPityBonus = {
  pityBonusMul: number;
  pityBonusAdd: number;
  realElo: number;
  pityElo: number;
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
  discordInfo: DiscordPlayerInfo[],
): PlayerAvatar[] {
  return discordInfo.map((e) => {
    return { username: e.username, avatarUrl: e.avatarUrl };
  });
}

export type Side = "left" | "right";
