import {
  Box,
  Button,
  FormControl,
  FormControlLabel,
  FormLabel,
  Modal,
  Radio,
  RadioGroup,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import type React from "react";
import { invoke } from "./Api";
import {
  isValidDurationString,
  parseDurationString,
  serializeDurationSeconds,
} from "./Duration";
import type { WinScale } from "./model";

export type FinishMatchModalState = {
  show: boolean;
  winner?: "left" | "right";
  fake?: boolean;
  duration?: string;
};

export function FinishMatchModal({
  state,
  setState,
}: {
  state: FinishMatchModalState;
  setState: React.Dispatch<React.SetStateAction<FinishMatchModalState>>;
}) {
  const sx = {
    position: "absolute",
    top: "50%",
    left: "50%",
    transform: "translate(-50%, -50%)",
    width: 400,
    bgcolor: "background.paper",
    boxShadow: 24,
    p: 4,
  };

  const userProvidedDurationInvalid = !isValidDurationString(state.duration);
  const showWinnerChoice = state.fake !== undefined && state.fake;
  const buttons: [WinScale, string][] = [
    ["pwnage", "Pwnage"],
    ["advantage", "Advantage"],
    ["even", "Even"],
  ];

  const winnerTeam = state.winner === "left" ? "Left Team" : "Right Team";
  const heading = state.fake
    ? "Enter fake result"
    : `${winnerTeam} won! How it went?`;
  const noWinner = state.winner === undefined;

  return (
    <Modal open={state.show} onClose={() => setState({ show: false })}>
      <Box sx={sx}>
        <Stack spacing={4}>
          <Typography variant="h6" component="h2">
            {heading}
          </Typography>
          {showWinnerChoice && (
            <FormControl>
              <FormLabel>Winner </FormLabel>
              <RadioGroup
                row
                onChange={(event) => {
                  setState((current) => {
                    return {
                      winner: event.target.value as "left" | "right",
                      ...current,
                    };
                  });
                }}
              >
                <FormControlLabel
                  value="left"
                  control={<Radio />}
                  label="Left Team"
                />
                <FormControlLabel
                  value="right"
                  control={<Radio />}
                  label="Right Team"
                />
              </RadioGroup>
            </FormControl>
          )}
          <TextField
            label="Duration"
            variant="standard"
            onChange={(event) => {
              setState((current) => {
                return {
                  ...current,
                  duration: event.target.value,
                };
              });
            }}
            error={userProvidedDurationInvalid}
            value={state.duration ?? "0m"}
          />
          <Stack spacing={2}>
            {buttons.map((b) => {
              const [scale, text] = b;
              return (
                <Button
                  key={scale}
                  variant="contained"
                  onClick={async () => {
                    await invoke("finish_match", {
                      winner: state.winner,
                      scale,
                      duration: serializeDurationSeconds(
                        parseDurationString(
                          state.duration === undefined ? "0" : state.duration,
                        ),
                      ),
                      fake: state.fake,
                    });
                    setState({ show: false });
                  }}
                  disabled={userProvidedDurationInvalid || noWinner}
                >
                  {text}
                </Button>
              );
            })}
          </Stack>
        </Stack>
      </Box>
    </Modal>
  );
}
