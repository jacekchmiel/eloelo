import { useMediaQuery } from "@mui/material";
import React from "react";

export const usePreferredColorScheme = () => {
	const getPreferredColorScheme = (): "dark" | "light" => {
		const prefersDarkMode = useMediaQuery("(prefers-color-scheme: dark)");
		return prefersDarkMode ? "dark" : "light";
	};

	const resetApplicationLoadingBackgroundStyles = () => {
		React.useEffect(() => {
			document.body.className = "";
		}, []);
	};

	return {
		getPreferredColorScheme,
		resetApplicationLoadingBackgroundStyles,
	};
};
