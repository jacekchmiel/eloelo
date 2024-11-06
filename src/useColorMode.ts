import { useMediaQuery } from "@mui/material";
import React, { useCallback } from "react";

export const useColorMode = () => {
	const getPreferredColorScheme = (): "dark" | "light" => {
		const prefersDarkMode = useMediaQuery("(prefers-color-scheme: dark)");
		return prefersDarkMode ? "dark" : "light";
	};

	const [mode, setMode] = React.useState(getPreferredColorScheme());

	const toggleColorMode = useCallback(() => {
		setMode((prevMode) => (prevMode === "light" ? "dark" : "light"));
	}, []);

	return React.useMemo(
		() => ({
			mode,
			colorMode: { toggleColorMode },
		}),
		[mode, toggleColorMode],
	);
};
