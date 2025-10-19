import { PropaneRounded } from "@mui/icons-material";
import ArrowDropDownIcon from "@mui/icons-material/ArrowDropDown";
import {
  Box,
  Button,
  ButtonGroup,
  type ButtonProps,
  ClickAwayListener,
  Grow,
  Paper,
  Popper,
  Stack,
} from "@mui/material";
import type { PropsWithChildren } from "react";
import React from "react";

interface SplitButtonProps extends ButtonProps {
  label: string;
}
export function SplitButton({
  label,
  children,
  ...props
}: PropsWithChildren<SplitButtonProps>) {
  console.log(props);
  const [open, setOpen] = React.useState(false);
  const anchorRef = React.useRef<HTMLDivElement>(null);

  const handleToggle = () => {
    setOpen((prevOpen) => !prevOpen);
  };

  const handleClose = (event: Event) => {
    if (anchorRef.current?.contains(event.target as HTMLElement)) {
      return;
    }
    setOpen(false);
  };
  return (
    <Box>
      <ButtonGroup variant="contained" ref={anchorRef}>
        <Button {...props}>{label}</Button>
        <Button size="small" onClick={handleToggle}>
          <ArrowDropDownIcon />
        </Button>
      </ButtonGroup>
      <Popper
        sx={{
          zIndex: 1,
          width: anchorRef.current ? anchorRef.current.clientWidth : "auto",
        }}
        open={open}
        anchorEl={anchorRef.current}
        transition
        disablePortal
        onClick={() => setOpen(false)}
      >
        {({ TransitionProps, placement }) => (
          <Grow
            {...TransitionProps}
            style={{
              transformOrigin:
                placement === "bottom" ? "center top" : "center bottom",
            }}
          >
            <Paper>
              <ClickAwayListener onClickAway={handleClose}>
                <Stack>
                  {React.Children.map(children, (child, _index) => child)}
                </Stack>
              </ClickAwayListener>
            </Paper>
          </Grow>
        )}
      </Popper>
    </Box>
  );
}
