import {
  FormControlLabel,
  Stack,
  Switch,
  TextField,
  Typography,
  styled,
} from "@mui/material";
import type { DescribedOption, OptionsGroup } from "../model";

type Value = boolean | number | string;

export function makeGenericOptions(options: OptionsGroup[]): GenericOptions {
  const generic = Object.fromEntries(
    options.map((s) => [
      s.key,
      Object.fromEntries(s.options.map((opt) => [opt.key, opt.value])),
    ]),
  );
  console.log({ generic });
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

type OptionInputProps = {
  opt: DescribedOption;
  value: Value;
  setValue: (newValue: Value) => void;
};

function OptionInput({ opt, value, setValue }: OptionInputProps) {
  if (opt.type === "boolean") {
    return <FormControlLabel control={<Switch />} label={opt.name} />;
  }
  let fieldType = "text";
  if (opt.type === "decimal" || opt.type === "integer") {
    fieldType = "number";
  }
  console.log({ fieldType, opt });
  return (
    <TextFieldNoSpinButtons
      key={opt.key}
      label={opt.name}
      variant="outlined"
      size="small"
      type={fieldType}
      value={value}
      onChange={(event) => setValue(event.target.value)}
    />
  );
}

export function OptionsView({
  options,
  values,
  setValues,
}: {
  options: OptionsGroup[];
  values: GenericOptions;
  setValues: (mod: (current: GenericOptions) => GenericOptions) => void;
}) {
  console.log({ options, values, setValues });
  return (
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
  );
}
