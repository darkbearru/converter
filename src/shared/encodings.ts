export const manualEncodings = [
  { label: 'UTF-8', value: 'utf-8' },
  { label: 'UTF-16 LE', value: 'utf-16le' },
  { label: 'UTF-16 BE', value: 'utf-16be' },
  ...Array.from({ length: 9 }, (_, index) => ({
    label: `Windows-${1250 + index}`,
    value: `windows-${1250 + index}`,
  })),
  { label: 'Windows-874', value: 'windows-874' },
];
