import type { DiscordPlayerInfo, EloEloState } from "./model";
import { parseEloEloState } from "./parse";

export const invoke = async (command: string, args: object) => {
	console.info({ command, args });
	const url = `${location.href}api/v1/${command}`;
	const response = await fetch(url, {
		method: "POST",
		body: JSON.stringify(args),
		headers: new Headers({ "Content-Type": "application/json; charset=utf-8" }),
	});
	const body = await response.json();
	if (!response.ok) {
		const status = response.status;
		console.error({ status, body });
	}
};

export async function connectToUiStream(options: {
	onError: (error: string) => void;
	onUiState: (state: EloEloState) => void;
	onDiscordInfo: (info: DiscordPlayerInfo[]) => void;
}) {
	const ws = new WebSocket(`${location.href}api/v1/ui_stream`);

	ws.onmessage = (event) => {
		const uiStreamPayload = JSON.parse(event.data);
		console.info({ uiStreamPayload });
		if (uiStreamPayload.error) {
			options.onError(uiStreamPayload.error);
		}
		if (uiStreamPayload.success.state) {
			const parsed = parseEloEloState(uiStreamPayload.success.state);
			options.onUiState(parsed);
		}
		if (uiStreamPayload.success.discordInfo) {
			options.onDiscordInfo(uiStreamPayload.success.discordInfo);
		}
	};

	ws.onerror = (error) => {
		options.onError(`Websocket error: ${error}`);
	};

	return () => {
		ws.close();
	};
}
