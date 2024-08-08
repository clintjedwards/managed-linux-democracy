async function updateVotes() {
  try {
    const response = await fetch("http://10.100.7.120:8080/api/votes");
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
