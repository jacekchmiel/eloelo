import React from "react";
import type { EloEloState } from "./model";

import HistoryIcon from "@mui/icons-material/History";
import MenuIcon from "@mui/icons-material/Menu";
import RefreshIcon from "@mui/icons-material/Refresh";
import SettingsIcon from "@mui/icons-material/Settings";
import AppBar from "@mui/material/AppBar";
import Box from "@mui/material/Box";
import FormControl from "@mui/material/FormControl";
import IconButton from "@mui/material/IconButton";
import InputLabel from "@mui/material/InputLabel";
import ListItemIcon from "@mui/material/ListItemIcon";
import ListItemText from "@mui/material/ListItemText";
import Menu from "@mui/material/Menu";
import MenuItem from "@mui/material/MenuItem";
import Select, { type SelectChangeEvent } from "@mui/material/Select";
import Toolbar from "@mui/material/Toolbar";
import Typography from "@mui/material/Typography";
import { invoke } from "./Api";
import { ThemeSwitcher } from "./components/ThemeSwitcher";

function GameSelector({
  selectedGame,
  availableGames,
  disabled,
}: {
  selectedGame: string;
  availableGames: string[];
  disabled: boolean;
}) {
  const handleChange = async (event: SelectChangeEvent<string>) => {
    await invoke("change_game", { name: event.target.value });
  };

  return (
    <FormControl sx={{ m: 1, minWidth: 120 }} fullWidth size="small">
      <InputLabel>Game</InputLabel>
      <Select
        disabled={disabled}
        value={selectedGame}
        label="Game"
        onChange={handleChange}
      >
        {availableGames.map((game) => (
          <MenuItem value={game} key={game}>
            {game}
          </MenuItem>
        ))}
      </Select>
    </FormControl>
  );
}

export function EloEloAppBar({
  state,
  setShowMatchHistory,
  setShowSettings,
}: {
  state: EloEloState;
  setShowMatchHistory: (show: boolean) => void;
  setShowSettings: (show: boolean) => void;
}) {
  const [anchorEl, setAnchorEl] = React.useState<null | HTMLElement>(null);
  const isDrawerOpen = Boolean(anchorEl);

  const openDrawer = (event: React.MouseEvent<HTMLElement>) => {
    setAnchorEl(event.currentTarget);
  };

  const closeDrawerAnd = (action: () => void) => {
    return () => {
      setAnchorEl(null);
      action();
    };
  };

  const renderDrawer = (
    <Menu
      anchorEl={anchorEl}
      id="hamburger-menu"
      keepMounted
      open={isDrawerOpen}
      onClose={closeDrawerAnd(() => {})}
    >
      <MenuItem onClick={closeDrawerAnd(async () => setShowMatchHistory(true))}>
        <ListItemIcon>
          <HistoryIcon />
        </ListItemIcon>
        <ListItemText>View Matches History</ListItemText>
      </MenuItem>

      <MenuItem
        onClick={closeDrawerAnd(async () => await invoke("refresh_elo", {}))}
      >
        <ListItemIcon>
          <RefreshIcon />
        </ListItemIcon>
        <ListItemText>Refresh Elo</ListItemText>
      </MenuItem>

      <MenuItem onClick={closeDrawerAnd(async () => setShowSettings(true))}>
        <ListItemIcon>
          <SettingsIcon />
        </ListItemIcon>
        <ListItemText>Settings</ListItemText>
      </MenuItem>
    </Menu>
  );

  return (
    <Box sx={{ flexGrow: 1 }}>
      <AppBar position="static">
        <Toolbar>
          <IconButton
            size="large"
            edge="start"
            color="inherit"
            onClick={openDrawer}
            sx={{ mr: 2 }}
          >
            <MenuIcon />
          </IconButton>
          <Typography
            variant="h5"
            noWrap
            component="div"
            sx={{ display: { xs: "none", sm: "block" } }}
          >
            Elo Elo
          </Typography>
          <Box sx={{ flexGrow: 1 }} />
          <Box sx={{ mr: 3 }}>
            <GameSelector
              availableGames={state.availableGames.map((g) => g.name)}
              selectedGame={state.selectedGame}
              disabled={state.gameState === "matchInProgress"}
            />
          </Box>
          <ThemeSwitcher />
        </Toolbar>
      </AppBar>
      {renderDrawer}
    </Box>
  );
}
