import { createPortal } from "react-dom";
import { Button } from "./Button";
import { Icon } from "./Icon";
import { SelectMenu } from "./SelectMenu";
import { useSelect, type SelectProps } from "./use-select";

export type { SelectOption, SelectProps } from "./use-select";

export function Select(props: SelectProps) {
  const model = useSelect(props);
  const selected = props.options.find(option => option.value === props.value);
  return <span className="ccc-select">
    <Button id={props.id} ref={model.trigger} variant="field" size={props.size} className="ccc-select-trigger"
      role="combobox" aria-label={props.label} aria-expanded={model.open} aria-haspopup="listbox"
      aria-controls={model.open ? model.listId : undefined} aria-activedescendant={model.open ? model.activeId : undefined}
      data-placeholder={selected === undefined || undefined}
      aria-disabled={props.disabled || props.busy || undefined} aria-busy={props.busy && !props.disabled || undefined}
      tabIndex={props.disabled ? -1 : 0} onClick={model.toggle} onKeyDown={model.keyDown}>
      <span style={selected?.style}>{selected?.label ?? props.placeholder ?? "请选择"}</span>
      <Icon name="chevron-down" size={16} />
    </Button>
    {model.open && createPortal(<SelectMenu props={props} model={model} />, model.trigger.current?.closest("dialog") ?? document.body)}
  </span>;
}
