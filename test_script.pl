$| = 1;
print {STDERR} "stderr stream\n";

for my $j (1..10) {
	for my $i (1..10) {
		print "x: $j - $i\n";
	}
	sleep 1;
}

print {STDERR} "stderr stream 2\n";
print $ARGV[0], "\n";
