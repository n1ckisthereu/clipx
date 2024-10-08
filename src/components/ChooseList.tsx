export interface Choose {
  name: string;
  link: string;
  class: string;
}

export function ChooseList({ choices }: { choices: Choose[] }) {
  return (
    <>
      {choices.map((choice, index) => (
        <ul>
          <li>
            <a key={index} href={choice.link} className={choice.class}>
              {choice.name}
            </a>
          </li>
        </ul>
      ))}
    </>
  );
}
