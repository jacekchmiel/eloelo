import Box from "@mui/material/Box";
import Modal from "@mui/material/Modal";
import type { PropsWithChildren } from "react";

export type DefaultModalProps = {
  show: boolean;
  setShow: (show: boolean) => void;
  size?: "large" | "small";
};

export function DefaultModal({
  show,
  setShow,
  size,
  children,
}: PropsWithChildren<DefaultModalProps>) {
  let top = "15%";
  if (size === "large") top = "5%";
  return (
    <Modal open={show} onClose={() => setShow(false)}>
      <Box
        sx={{
          position: "absolute" as const,
          top,
          left: "50%",
          transform: "translate(-50%, 0%)",
          bgcolor: "background.paper",
          boxShadow: 24,
          p: 4,
          overflowY: "auto",
          // Keep the bottom edge at 95% of the window, whatever the top.
          maxHeight: `calc(95% - ${top})`,
        }}
      >
        {children}
      </Box>
    </Modal>
  );
}
