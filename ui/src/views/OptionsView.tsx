import {
  Button,
  DialogActions,
  FormControlLabel,
  Stack,
  Switch,
  TextField,
  Typography,
  styled,
} from "@mui/material";
import type { DescribedOption, OptionType, OptionsGroup } from "../model";

type Value = boolean | number | string;

export function makeGenericOptions(options: OptionsGroup[]): GenericOptions {
  const generic = Object.fromEntries(
    options.map((s) => [
      s.key,
      Object.fromEntries(s.options.map((opt) => [opt.key, opt.value])),
    ]),
  );
  return generic;
}

export type GenericOptions = {
  [group: string]: { [key: string]: Value };
};

const TextFieldNoSpinButtons = styled(TextField)({
  "& input[type=number]": {
    // Hide arrows for Chrome, Safari, Edge, Opera
    "&::-webkit-outer-spin-button, &::-webkit-inner-spin-button": {
      "-webkit-appearance": "none",
      margin: 0,
    },
    // Hide arrows for Firefox
    "-moz-appearance": "textfield",
  },
});

function castTextFieldValue(type: OptionType, value: string): Value {
  switch (type) {
    case "decimal":
      return Number.parseFloat(value);
    case "integer":
      return Number.parseInt(value);
    case "string":
      return value;
    default:
      return "INVALID";
  }
}

type OptionInputProps = {
  opt: DescribedOption;
  value: Value;
  setValue: (newValue: Value) => void;
};

function textInput({ opt, value, setValue }: OptionInputProps) {
  let fieldType = "text";
  if (opt.type === "decimal" || opt.type === "integer") {
    fieldType = "number";
  }
  return (
    <TextFieldNoSpinButtons
      key={opt.key}
      label={opt.name}
      variant="outlined"
      size="small"
      type={fieldType}
      value={value}
      onChange={(event) =>
        setValue(castTextFieldValue(opt.type, event.target.value))
      }
    />
  );
}

function boolInput({ opt, value, setValue }: OptionInputProps) {
  if (typeof value !== "boolean") {
    return <>Invalid value type</>;
  }
  return (
    <FormControlLabel
      control={
        <Switch
          checked={value}
          onChange={(event) => setValue(event.target.checked)}
        />
      }
      label={opt.name}
    />
  );
}

function OptionInput(props: OptionInputProps) {
  if (props.opt.type === "boolean") {
    return boolInput(props);
  }
  return textInput(props);
}

export function OptionsView({
  options,
  values,
  setValues,
  onSave,
  onCancel,
}: {
  options: OptionsGroup[];
  values: GenericOptions;
  setValues: (mod: (current: GenericOptions) => GenericOptions) => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  return (
    <>
      <Stack spacing={2} minWidth={300}>
        {options.map((group: OptionsGroup) => (
          <>
            <Typography variant="h6" key={group.key}>
              {group.name}
            </Typography>
            {group.options.map((opt: DescribedOption) => (
              <OptionInput
                key={opt.key}
                opt={opt}
                value={values[group.key][opt.key]}
                setValue={(value: Value) => {
                  setValues((current) => {
                    return {
                      ...current,
                      [group.key]: {
                        ...current[group.key],
                        [opt.key]: value,
                      },
                    };
                  });
                }}
              />
            ))}
          </>
        ))}
      </Stack>
      <DialogActions>
        <Button onClick={onCancel}>Cancel</Button>
        <Button onClick={onSave}>Save</Button>
      </DialogActions>
    </>
  );
}
