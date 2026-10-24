phonemes {
    consonants {
        phoneme "k" {
            roman "k"
        }
        phoneme "s" {
            roman "s"
        }
        phoneme "t" {
            roman "t"
            when {
                word-position "initial"
                next-phoneme "a"
                group {
                    syllable-position "coda"
                    previous-phoneme "s"
                }
                or {
                    previous-phoneme "z"
                    next-phoneme "k"
                }
            }
        }
        phoneme "z" {
            roman "z"
        }
    }
    vowels {
        phoneme "a" {
            roman "a"
        }
    }
}
