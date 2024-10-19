import AddIcon from "@mui/icons-material/Add";
import DeleteIcon from "@mui/icons-material/Delete";
import EditIcon from "@mui/icons-material/Edit";
import EditOffIcon from "@mui/icons-material/EditOff";
import PersonIcon from "@mui/icons-material/Person";
import PersonAddIcon from "@mui/icons-material/PersonAdd";
import PersonAddAlt1Icon from "@mui/icons-material/PersonAddAlt1";
import {
	Autocomplete,
	Avatar,
	Box,
	IconButton,
	type IconButtonProps,
	List,
	ListItem,
	ListItemAvatar,
	ListItemText,
	Paper,
	Stack,
	TextField,
	styled,
} from "@mui/material";
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import type { Avatars, Player, PlayerAvatar } from "./model";

const Header = styled(Box)(({ theme }) => ({
	...theme.typography.h6,
	textAlign: "left",
	color: theme.palette.text.primary,
}));

function DeleteButton({
	playerKey,
	...props
}: { playerKey: string } & IconButtonProps) {
	return (
		<IconButton
			{...props}
			edge="end"
			onClick={async () => await invoke("remove_player", { name: playerKey })}
		>
			<DeleteIcon />
		</IconButton>
	);
}

function EditButton({
	editable,
	...props
}: { editable: boolean } & IconButtonProps) {
	return (
		<IconButton {...props}>
			{editable ? <EditOffIcon /> : <EditIcon />}
		</IconButton>
	);
}

function AddLeftButton({
	playerKey,
	...props
}: { playerKey: string } & IconButtonProps) {
	return (
		<IconButton
			{...props}
			edge="end"
			aria-label="add_left"
			onClick={async () =>
				await invoke("add_player_to_team", { name: playerKey, team: "left" })
			}
		>
			<PersonAddIcon />
		</IconButton>
	);
}

function AddRightButton({
	playerKey,
	...props
}: { playerKey: string } & IconButtonProps) {
	return (
		<IconButton
			{...props}
			edge="end"
			aria-label="add_right"
			onClick={async () =>
				await invoke("add_player_to_team", { name: playerKey, team: "right" })
			}
		>
			<PersonAddAlt1Icon />
		</IconButton>
	);
}

function AddButton({ newPlayerName }: { newPlayerName: string | null }) {
	return (
		newPlayerName !== null && (
			<IconButton
				edge="end"
				aria-label="add"
				onClick={async () =>
					await invoke("add_new_player", { name: newPlayerName })
				}
			>
				<AddIcon />
			</IconButton>
		)
	);
}

function AvatarPlaceholder() {
	return (
		<ListItemAvatar>
			<Avatar>
				<PersonIcon />
			</Avatar>
		</ListItemAvatar>
	);
}

function NewPlayerRow({ players }: { players: string[] }) {
	const [newPlayerName, setNewPlayerName] = useState<string | null>(null);
	return (
		<ListItem sx={{ p: 0 }}>
			<AvatarPlaceholder />
			<Autocomplete
				sx={{ width: 300 }}
				freeSolo
				options={players}
				renderInput={(params) => (
					<TextField
						{...params}
						label="Add new player"
						onChange={(event) => {
							setNewPlayerName(event.target.value);
						}}
					/>
				)}
				onChange={(_, value) => {
					setNewPlayerName(value);
				}}
			/>
			<AddButton {...{ newPlayerName }} />
		</ListItem>
	);
}

export function ReserveList({
	players,
	assemblingTeams,
	avatars,
	playersToAdd,
}: {
	players: Player[];
	assemblingTeams: boolean;
	avatars: Avatars;
	playersToAdd: string[];
}) {
	const [editable, setEditable] = useState(() => false);

	const playerEntries = players.map((player) => {
		const avatarUrl = avatars.find(
			(a: PlayerAvatar) => a.player === player.name,
		)?.avatarUrl;
		return (
			<ListItem key={player.name} sx={{ p: 0 }}>
				{/* <MoveButton playerKey={name} /> */}
				<ListItemAvatar>
					<Avatar src={avatarUrl} />
				</ListItemAvatar>
				<ListItemText primary={player.name} secondary={player.elo} />
				<AddLeftButton playerKey={player.name} disabled={!assemblingTeams} />
				<AddRightButton playerKey={player.name} disabled={!assemblingTeams} />
				{editable && <DeleteButton playerKey={player.name} />}
			</ListItem>
		);
	});
	return (
		<Paper>
			<Stack sx={{ p: 2 }}>
				<Stack direction="row" justifyContent="space-between">
					<Header>Reserve</Header>
					<Stack direction="row" justifyContent="flex-end">
						{/* {editable && <DownloadButton />} */}
						<EditButton
							editable={editable}
							onClick={() => setEditable((prev) => !prev)}
						/>
					</Stack>
				</Stack>
				<List>
					{playerEntries}
					{editable && <NewPlayerRow players={playersToAdd} />}
				</List>
			</Stack>
		</Paper>
	);
}
