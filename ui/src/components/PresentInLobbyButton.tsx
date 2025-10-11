import PersonIcon from "@mui/icons-material/Person";
import PersonOffIcon from "@mui/icons-material/PersonOff";
import { IconButton, type IconButtonProps } from "@mui/material";
import { invoke } from "../Api";
import type { Side } from "../model";
import { DefaultTooltip } from "./DefaultTooltip";

export function PresentInLobbyButton({
	side,
	playerKey,
	present,
	...props
}: { side: Side; playerKey: string; present: boolean } & IconButtonProps) {
	return (
		<DefaultTooltip title="Toggle lobby presence">
			<IconButton
				{...props}
				edge={side === "left" ? "start" : "end"}
				onClick={async () => {
					await invoke("present_in_lobby_change", {
						id: playerKey,
						present: !present,
					});
				}}
			>
				{present ? (
					<PersonIcon color="success" />
				) : (
					<PersonOffIcon color="error" />
				)}
			</IconButton>
		</ DefaultTooltip>
	);
}
