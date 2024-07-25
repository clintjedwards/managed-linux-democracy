async function updateCurlCommand() {
  try {
    const response = await fetch("http://localhost:8080/api/system");
    if (!response.ok) {
      throw new Error("Network response was not ok " + response.statusText);
    }
    const data = await response.json();
    const address = data.address;

    const curlCommandElement = document.getElementById("curl-command");
    const currentCommand = curlCommandElement.textContent;
    const updatedCommand = currentCommand.replace("localhost", address);

    curlCommandElement.textContent = updatedCommand;
  } catch (error) {
    console.error("There has been a problem with your fetch operation:", error);
  }
}

updateCurlCommand();

async function updateVotes() {
  try {
    const response = await fetch("http://localhost:8080/api/votes");
    if (!response.ok) {
      throw new Error("Network response was not ok " + response.statusText);
    }
    const data = await response.json();
    const votes = data.votes;

    const rows = document.querySelectorAll(".chart tbody tr");
    votes.forEach((vote, index) => {
      const label = vote[0];
      const value = vote[1];
      const percentage = (value / votes.reduce((sum, vote) => sum + vote[1], 0)) * 100 || 0;

      rows[index].querySelector("th").textContent = label;
      rows[index].querySelector("td").style.setProperty("--size", percentage / 100);
      rows[index].querySelector("td").textContent = `${percentage.toFixed(1)}%`;
    });
  } catch (error) {
    console.error("There has been a problem with your fetch operation:", error);
  }
}

setInterval(updateVotes, 500);
