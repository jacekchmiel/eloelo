import { IconButton } from "@mui/material";
import { useTheme } from "@mui/material/styles";
import React from "react";
import Brightness4Icon from "@mui/icons-material/Brightness4";
import Brightness7Icon from "@mui/icons-material/Brightness7";

export const ColorModeContext = React.createContext<{
	toggleColorMode: () => void;
}>({
	toggleColorMode: () => {
		throw new Error("ColorModeContext unavailable");
	},
});

export function ThemeSwitcher() {
	const theme = useTheme();
	const colorMode = React.useContext(ColorModeContext);

	return (
		<IconButton
			sx={{ ml: 1 }}
			onClick={colorMode.toggleColorMode}
			color="inherit"
		>
			{theme.palette.mode === "dark" ? (
				<Brightness7Icon />
			) : (
				<Brightness4Icon />
			)}
		</IconButton>
	);
}
