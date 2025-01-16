import { IconButton, type IconButtonProps } from "@mui/material";
import type { Side } from "../model";
import { invoke } from "@tauri-apps/api/core";
import PersonIcon from "@mui/icons-material/Person";
import PersonOffIcon from "@mui/icons-material/PersonOff";

export function PresentInLobbyButton({
	side,
	playerKey,
	present,
	...props
}: { side: Side; playerKey: string; present: boolean } & IconButtonProps) {
	return (
		<IconButton
			{...props}
			edge={side === "left" ? "start" : "end"}
			aria-label="delete"
			onClick={async () =>
				await invoke("present_in_lobby_change", {
					id: playerKey,
					present: !present,
				})
			}
		>
			{present ? (
				<PersonIcon color="success" />
			) : (
				<PersonOffIcon color="error" />
			)}
		</IconButton>
	);
}
