use ezlz::t;

#[test]
fn plural_languages() {
    ezlz::init("test", "tests/locales").unwrap();
    let ru = [
        (0, "0 столов"),
        (1, "1 стол"),
        (2, "2 стола"),
        (4, "4 стола"),
        (5, "5 столов"),
        (10, "10 столов"),
        (11, "11 столов"),
        (12, "12 столов"),
        (14, "14 столов"),
        (15, "15 столов"),
        (20, "20 столов"),
        (21, "21 стол"),
        (22, "22 стола"),
        (24, "24 стола"),
        (25, "25 столов"),
        (101, "101 стол"),
        (102, "102 стола"),
        (104, "104 стола"),
        (105, "105 столов"),
        (111, "111 столов"),
        (112, "112 столов"),
        (114, "114 столов"),
        (115, "115 столов"),
        (1001, "1001 стол"),
        (1002, "1002 стола"),
        (1004, "1004 стола"),
        (1005, "1005 столов"),
        (1011, "1011 столов"),
        (1012, "1012 столов"),
        (1014, "1014 столов"),
        (1015, "1015 столов"),
        (-1, "-1 стол"),
        (-2, "-2 стола"),
        (-5, "-5 столов"),
        (-11, "-11 столов"),
        (-21, "-21 стол"),
    ];
    let ru_float = [
        (1.5, "1.5 стола"),
        (2.5, "2.5 стола"),
        (11.5, "11.5 стола"),
        (101.5, "101.5 стола"),
    ];

    // English
    let en = [
        (-2, "-2 foxes"),
        (-1, "-1 fox"),
        (0, "0 foxes"),
        (1, "1 fox"),
        (2, "2 foxes"),
        (10, "10 foxes"),
        (101, "101 foxes"),
    ];
    let en_float = [(1.5, "1.5 foxes"), (-1.5, "-1.5 foxes")];

    // Romanian
    let ro = [
        (-2, "-2 vulpi"),
        (-1, "-1 vulpe"),
        (0, "0 vulpi"),
        (1, "1 vulpe"),
        (2, "2 vulpi"),
        (3, "3 vulpi"),
        (4, "4 vulpi"),
        (5, "5 vulpi"),
        (10, "10 vulpi"),
        (11, "11 vulpi"),
        (20, "20 de vulpi"),
        (21, "21 de vulpi"),
        (101, "101 vulpi"),
        (119, "119 vulpi"),
        (122, "122 de vulpi"),
    ];
    let ro_float = [(1.5, "1.5 de vulpi"), (11.5, "11.5 de vulpi")];

    // Arabic
    let ar = [
        (0, "0 zero"),
        (1, "1 one"),
        (-1, "-1 one"),
        (101, "101 one"),
        (201, "201 one"),
        (2, "2 two"),
        (-2, "-2 two"),
        (102, "102 two"),
        (202, "202 two"),
        (3, "3 few"),
        (10, "10 few"),
        (103, "103 few"),
        (110, "110 few"),
        (11, "11 many"),
        (12, "12 many"),
        (20, "20 many"),
        (99, "99 many"),
        (111, "111 many"),
        (199, "199 many"),
        (100, "100 other"),
        (200, "200 other"),
        (1000, "1000 other"),
    ];
    let ar_float = [(1.5, "1.5 other"), (2.5, "2.5 other"), (-1.5, "-1.5 other")];

    // French
    let fr = [
        (-2, "-2 articles"),
        (-1, "-1 article"),
        (0, "0 article"),
        (1, "1 article"),
        (2, "2 articles"),
        (10, "10 articles"),
        (101, "101 articles"),
    ];
    let fr_float = [
        (-0.5, "-0.5 article"),
        (1.5, "1.5 article"),
        (2.0, "2.0 articles"),
    ];

    // Chinese: intentionally no plural distinction.
    let cn = [
        (-2, "-2 件"),
        (-1, "-1 件"),
        (0, "0 件"),
        (1, "1 件"),
        (2, "2 件"),
        (10, "10 件"),
        (101, "101 件"),
        (1001, "1001 件"),
    ];
    let cn_float = [(-1.5, "-1.5 件"), (1.5, "1.5 件")];

    // German
    let de = [
        (-2, "-2 Füchse"),
        (-1, "-1 Fuchs"),
        (0, "0 Füchse"),
        (1, "1 Fuchs"),
        (2, "2 Füchse"),
        (10, "10 Füchse"),
        (101, "101 Füchse"),
    ];
    let de_float = [(1.5, "1.5 Füchse"), (-1.5, "-1.5 Füchse")];

    for (n, expected) in ru {
        assert_eq!(t!("test", test.ru, i = n), expected);
    }
    for (n, expected) in ru_float {
        assert_eq!(t!("test", test.ru, i = n), expected);
    }

    for (n, expected) in en {
        assert_eq!(t!("test", test.en, i = n), expected);
    }
    for (n, expected) in en_float {
        assert_eq!(t!("test", test.en, i = n), expected);
    }

    for (n, expected) in ro {
        assert_eq!(t!("test", test.ro, i = n), expected);
    }
    for (n, expected) in ro_float {
        assert_eq!(t!("test", test.ro, i = n), expected);
    }

    for (n, expected) in ar {
        assert_eq!(t!("test", test.ar, i = n), expected);
    }
    for (n, expected) in ar_float {
        assert_eq!(t!("test", test.ar, i = n), expected);
    }

    for (n, expected) in fr {
        assert_eq!(t!("test", test.fr, i = n), expected);
    }
    for (n, expected) in fr_float {
        assert_eq!(t!("test", test.fr, i = n), expected);
    }

    for (n, expected) in cn {
        assert_eq!(t!("test", test.cn, i = n), expected);
    }
    for (n, expected) in cn_float {
        assert_eq!(t!("test", test.cn, i = n), expected);
    }

    for (n, expected) in de {
        assert_eq!(t!("test", test.de, i = n), expected);
    }
    for (n, expected) in de_float {
        assert_eq!(t!("test", test.de, i = n), expected);
    }
}
