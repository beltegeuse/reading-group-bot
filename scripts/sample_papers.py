import random
import argparse
import csv 

# Create a commande line how will take a csv file as input
parser = argparse.ArgumentParser(description='Generate a random sample of papers from a csv file')
parser.add_argument('file', metavar='file', type=str, help='csv file to sample from')
parser.add_argument('n', metavar='n', type=int, help='number of papers to sample')
parser.add_argument('a', metavar='a', type=str, help='csv file to role list')
args = parser.parse_args()

# Read the csv file and store the lines in a list
papers = []
with open(args.file, 'r', encoding="utf8") as f:
    print(f"Reading file: {args.file}")
    paperread = csv.DictReader(f, delimiter=',')
    i = 0
    for row in paperread:
        i += 1
        if i==1:
            continue # Skip the header

        papers.append(row)

print(f"Read {len(papers)} papers")

selected_paper = []
authors = set(())
for _ in range(args.n):
    paper = random.choice(papers)
    while paper['Suggested by'].strip() in authors:
        paper = random.choice(papers)
    
    print(f"{paper['Paper title']} - {paper['Suggested by']}")
    authors.add(paper['Suggested by'].strip())
    selected_paper.append(paper)

command = '/simplepoll "Next paper for our reading group, you can vote to any paper you like. Multiple vote are allowed. You have until next Friday to vote."'
for paper in selected_paper:
    command += f' "{paper["Paper title"]} - {paper["Suggested by"]}"'
print(command)    

# Read the csv file containing authors and store the lines in a list
authors = []
with open(args.a, 'r', encoding="utf8") as f:
    print(f"Reading file: {args.a}")
    authors = [l.replace(",", "").strip() for l in f.readlines() ]

authors = list(authors)
random.shuffle(authors)
print(f"Authors: {authors}")