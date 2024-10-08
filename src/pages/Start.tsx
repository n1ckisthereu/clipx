import { Choose, ChooseList } from "../components/ChooseList";

function Start() {
  const choicesList: Choose[] = [
    { name: "Create a server", link: "/create", class: "nav-item" },
    { name: "Connect to an existing one", link: "/connect", class: "nav-item" },
  ];

  return (
    <div>
      <ChooseList choices={choicesList} />
    </div>
  );
}

export default Start;
