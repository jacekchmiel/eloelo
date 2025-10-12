import Tooltip, { type TooltipProps } from "@mui/material/Tooltip";

export function DefaultTooltip(props: TooltipProps) {
  return <Tooltip arrow disableInteractive {...props} />;
}
